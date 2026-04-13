#!/usr/bin/env node

/**
 * resolve-account.cjs — imap-smtp-email 账号解析入口
 *
 * 在执行 smtp.js / imap.js 之前，先解析要使用的邮箱账号：
 *
 *   1. 调 4230 接口（authorized-email-platforms）拉用户全部绑定的私人邮箱
 *   2. 邮箱选择逻辑：
 *      - 用户命令中通过 --account-email 明确指定了邮箱 → 使用该邮箱
 *      - 未指定 → 从绑定列表中选一个（排除 public_mail）
 *      - 没有绑定 → 返回错误提示需要先绑定
 *   3. 调 get-token.sh --platform xxx 写入 .env
 *   4. 继续执行原有命令
 *
 * 用法：
 *   node resolve-account.cjs send [--account-email your@163.com] --to ... --subject ... --body ...
 *   node resolve-account.cjs check [--account-email your@163.com] --limit 10
 *   node resolve-account.cjs resolve [--account-email your@163.com]   ← 仅解析，不执行后续命令
 */

const fs = require('fs');
const path = require('path');
const http = require('http');
const { spawnSync, spawn } = require('child_process');

const SKILL_DIR = path.resolve(__dirname, '..');
const GET_TOKEN_SH = path.join(SKILL_DIR, 'get-token.sh');
const SMTP_JS = path.join(__dirname, 'smtp.js');
const IMAP_JS = path.join(__dirname, 'imap.js');
const ENV_FILE = path.join(SKILL_DIR, '.env');

// 个人邮箱平台白名单（与拦截器一致，排除 public_mail 等非个人邮箱）
const PERSONAL_PLATFORMS = new Set([
  '163_mail', 'qq_mail', 'gmail', 'outlook', 'sina_mail', 'sohu_mail',
]);

// platform → 邮箱域名（用于反查）
const PLATFORM_DOMAIN_MAP = {
  '163_mail': '163.com',
  'qq_mail': 'qq.com',
  'gmail': 'gmail.com',
  'outlook': 'outlook.com',
  'sina_mail': 'sina.com',
  'sohu_mail': 'sohu.com',
};

// 邮箱域名 → platform（用于正查）
const DOMAIN_PLATFORM_MAP = {
  '163.com': '163_mail',
  'vip.163.com': '163_mail',
  '126.com': '163_mail',
  'vip.126.com': '163_mail',
  '188.com': '163_mail',
  'vip.188.com': '163_mail',
  'yeah.net': '163_mail',
  'qq.com': 'qq_mail',
  'foxmail.com': 'qq_mail',
  'vip.qq.com': 'qq_mail',
  'gmail.com': 'gmail',
  'outlook.com': 'outlook',
  'sina.com': 'sina_mail',
  'sohu.com': 'sohu_mail',
};

const HTTP_REQUEST_TIMEOUT = 15000;

// ──────────────────────────────────────
// 通用工具
// ──────────────────────────────────────

function outputJson(payload) {
  process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`);
}

function success(payload = {}) {
  outputJson({ success: true, ...payload });
  process.exit(0);
}

function fail(message, errorCode = 1, extra = {}) {
  outputJson({ success: false, error_code: errorCode, message, ...extra });
  process.exit(1);
}

function getOptionValue(args, flag) {
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] === flag) {
      return args[i + 1] || '';
    }
  }
  return '';
}

function removeFlagWithValue(args, flag) {
  const result = [];
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] === flag) {
      i += 1; // skip value
      continue;
    }
    result.push(args[i]);
  }
  return result;
}

// ──────────────────────────────────────
// 调 4230 接口拉全部绑定的私人邮箱
// ──────────────────────────────────────

function getProxyPort() {
  const envPort = process.env.AUTH_GATEWAY_PORT;
  if (envPort) {
    const parsed = parseInt(envPort, 10);
    if (!isNaN(parsed) && parsed > 0) return parsed;
  }
  return 19000;
}

function getRemoteBaseUrl() {
  return 'https://jprx.m.qq.com';
}

/**
 * 通过 Auth Gateway 代理查询已绑定的个人邮箱列表
 *
 * @returns {Promise<Array<{platform: string, email: string, auth_type?: string}>>}
 */
async function fetchBoundPersonalEmails() {
  const proxyPort = getProxyPort();
  const remoteBaseUrl = getRemoteBaseUrl();
  const remoteUrl = `${remoteBaseUrl}/data/4230/forward`;

  const bodyStr = JSON.stringify({});

  return new Promise((resolve, reject) => {
    const request = http.request({
      host: '127.0.0.1',
      port: proxyPort,
      path: '/proxy/api',
      method: 'POST',
      timeout: HTTP_REQUEST_TIMEOUT,
      headers: {
        'Remote-URL': remoteUrl,
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(bodyStr),
      },
    }, (response) => {
      let rawBody = '';
      response.setEncoding('utf8');
      response.on('data', (chunk) => { rawBody += chunk; });
      response.on('end', () => {
        try {
          if (response.statusCode !== 200) {
            resolve([]);
            return;
          }
          const data = rawBody ? JSON.parse(rawBody) : null;
          if (!data || data.ret !== 0) {
            resolve([]);
            return;
          }
          // 兼容多种嵌套格式
          const respData = data?.data?.resp?.data ?? data?.data?.data ?? data?.data;
          const rawPlatforms = Array.isArray(respData?.platforms) ? respData.platforms : [];

          // 仅保留个人邮箱平台
          const personalEmails = rawPlatforms.filter(
            (p) => p.platform && PERSONAL_PLATFORMS.has(p.platform),
          );
          resolve(personalEmails);
        } catch (err) {
          resolve([]);
        }
      });
    });

    request.on('timeout', () => {
      request.destroy();
      resolve([]);
    });
    request.on('error', () => {
      resolve([]);
    });

    request.write(bodyStr);
    request.end();
  });
}

// ──────────────────────────────────────
// 调 get-token.sh 写入 .env
// ──────────────────────────────────────

/**
 * 根据选中的 platform 调 get-token.sh --platform xxx 写入 .env
 *
 * @param {string} platform - 凭证平台标识（如 "163_mail"）
 * @returns {{ success: boolean, message: string, email?: string }}
 */
function refreshCredentialForPlatform(platform) {
  if (!fs.existsSync(GET_TOKEN_SH)) {
    return { success: false, message: `未找到凭证刷新脚本: ${GET_TOKEN_SH}` };
  }

  try {
    const result = spawnSync('bash', [GET_TOKEN_SH, '--platform', platform], {
      cwd: SKILL_DIR,
      env: process.env,
      timeout: 15000,
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    const stdout = String(result.stdout || '').trim();
    const stderr = String(result.stderr || '').trim();

    if (result.error) {
      return { success: false, message: result.error.message || '凭证刷新脚本执行失败' };
    }

    // 尝试解析 stdout 中的 JSON
    let payload = null;
    try {
      payload = JSON.parse(stdout);
    } catch (e) {
      // stdout 不是 JSON
    }

    if ((result.status || 0) === 0 && fs.existsSync(ENV_FILE)) {
      return {
        success: true,
        message: payload?.message || '已刷新凭证',
        email: payload?.email || undefined,
      };
    }

    return {
      success: false,
      message: payload?.message || stderr || stdout || '凭证刷新失败',
    };
  } catch (err) {
    return { success: false, message: err.message || '凭证刷新异常' };
  }
}

// ──────────────────────────────────────
// 解析 .env 获取当前配置的邮箱
// ──────────────────────────────────────

function parseEnvFile(envPath) {
  if (!fs.existsSync(envPath)) return {};
  const content = fs.readFileSync(envPath, 'utf8');
  const env = {};
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    const sep = line.indexOf('=');
    if (sep === -1) continue;
    const key = line.slice(0, sep).trim();
    let value = line.slice(sep + 1).trim();
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1);
    }
    env[key] = value;
  }
  return env;
}

function getCurrentConfiguredEmail() {
  const env = parseEnvFile(ENV_FILE);
  return (env.SMTP_USER || env.SMTP_FROM || env.IMAP_USER || '').trim().toLowerCase();
}

// ──────────────────────────────────────
// 邮箱匹配逻辑
// ──────────────────────────────────────

/**
 * 根据邮箱地址找到对应的 platform
 */
function findPlatformForEmail(email, boundEmails) {
  const normalizedEmail = email.trim().toLowerCase();

  // 先从已绑定列表中精确匹配
  for (const item of boundEmails) {
    if (item.email && item.email.toLowerCase() === normalizedEmail) {
      return item.platform;
    }
  }

  // 回退：根据域名推断 platform
  const domain = normalizedEmail.split('@')[1];
  if (domain && DOMAIN_PLATFORM_MAP[domain]) {
    return DOMAIN_PLATFORM_MAP[domain];
  }

  return null;
}

// ──────────────────────────────────────
// IMAP 命令映射
// ──────────────────────────────────────

const IMAP_COMMANDS = new Set([
  'check', 'search', 'fetch', 'download',
  'mark-read', 'mark-unread', 'list-mailboxes',
]);

const SMTP_COMMANDS = new Set(['send', 'test']);

// ──────────────────────────────────────
// 主逻辑
// ──────────────────────────────────────

async function main() {
  const allArgs = process.argv.slice(2);
  const command = allArgs[0] || 'resolve';
  const restArgs = allArgs.slice(1);

  // 提取 --account-email 参数
  const accountEmail = getOptionValue(restArgs, '--account-email');
  const cleanArgs = removeFlagWithValue(restArgs, '--account-email');

  // 步骤 1：拉全部绑定的私人邮箱
  const boundEmails = await fetchBoundPersonalEmails();

  // 步骤 2：决定使用哪个邮箱
  let selectedPlatform = null;
  let selectedEmail = null;

  if (accountEmail) {
    // ── 用户明确指定了邮箱 ──
    selectedPlatform = findPlatformForEmail(accountEmail, boundEmails);
    selectedEmail = accountEmail.trim().toLowerCase();

    if (!selectedPlatform) {
      fail(
        `指定的邮箱 ${accountEmail} 无法匹配到已绑定的个人邮箱平台。请先在集成面板中绑定该邮箱。`,
        2,
        {
          requested_email: accountEmail,
          bound_emails: boundEmails.map((e) => ({ platform: e.platform, email: e.email })),
        },
      );
    }
  } else if (boundEmails.length === 0) {
    // ── 没有绑定任何邮箱 ──
    if (command === 'resolve') {
      // 仅解析模式，返回状态
      success({
        status: 'no_binding',
        message: '当前没有绑定任何个人邮箱，请先在集成面板中绑定邮箱。',
        bound_emails: [],
      });
    }
    fail(
      '当前没有绑定任何个人邮箱，请先在集成面板中绑定邮箱后重试。',
      2,
      { bound_emails: [] },
    );
  } else if (boundEmails.length === 1) {
    // ── 恰好 1 个绑定 → 自动选择 ──
    selectedPlatform = boundEmails[0].platform;
    selectedEmail = boundEmails[0].email;
  } else {
    // ── 多个绑定但未指定 → 选择第一个 ──
    // 注意：正常流程中如果有多个绑定，拦截器会弹选择卡片让用户选择后传入 --account-email
    // 这里是兜底逻辑
    selectedPlatform = boundEmails[0].platform;
    selectedEmail = boundEmails[0].email;
  }

  // 如果是 resolve 命令，仅输出解析结果
  if (command === 'resolve') {
    success({
      status: 'resolved',
      selected_platform: selectedPlatform,
      selected_email: selectedEmail,
      bound_emails: boundEmails.map((e) => ({ platform: e.platform, email: e.email })),
    });
  }

  // 步骤 3：检查当前 .env 配置的邮箱是否与选中的一致
  const currentEmail = getCurrentConfiguredEmail();
  const needRefresh = !currentEmail || currentEmail !== selectedEmail.toLowerCase();

  if (needRefresh) {
    // 调 get-token.sh --platform xxx 刷新凭证写入 .env
    const refreshResult = refreshCredentialForPlatform(selectedPlatform);
    if (!refreshResult.success) {
      fail(
        `切换到邮箱 ${selectedEmail} (${selectedPlatform}) 的凭证刷新失败：${refreshResult.message}`,
        2,
        {
          selected_platform: selectedPlatform,
          selected_email: selectedEmail,
          refresh_error: refreshResult.message,
        },
      );
    }
  }

  // 步骤 4：执行后续命令
  if (SMTP_COMMANDS.has(command)) {
    // 执行 smtp.js
    const child = spawn('node', [SMTP_JS, command, ...cleanArgs], {
      cwd: SKILL_DIR,
      env: process.env,
      stdio: 'inherit',
    });
    child.on('close', (code) => process.exit(code || 0));
    child.on('error', (err) => {
      fail(`执行 smtp.js 失败: ${err.message}`, 1);
    });
  } else if (IMAP_COMMANDS.has(command)) {
    // 执行 imap.js
    const child = spawn('node', [IMAP_JS, command, ...cleanArgs], {
      cwd: SKILL_DIR,
      env: process.env,
      stdio: 'inherit',
    });
    child.on('close', (code) => process.exit(code || 0));
    child.on('error', (err) => {
      fail(`执行 imap.js 失败: ${err.message}`, 1);
    });
  } else {
    fail(`未知命令: ${command}，支持的命令: send, test, check, search, fetch, download, mark-read, mark-unread, list-mailboxes, resolve`, 1);
  }
}

main().catch((err) => {
  fail(err.message || String(err), 999);
});
