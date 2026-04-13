#!/usr/bin/env node

/**
 * prosearch.js — ProSearch 联网搜索脚本
 *
 * 替代 curl 命令，通过 Node.js http.request 调用本地 Auth Gateway 代理，
 * 彻底避免 Windows PowerShell 下 curl 的 UTF-8 编码问题。
 *
 * 用法:
 *   node prosearch.js '{"keyword":"搜索关键词"}'
 *   node prosearch.js '{"keyword":"AI news","cnt":20}'
 *   node prosearch.js '{"keyword":"最新新闻","from_time":1710000000}'
 *
 * 环境变量:
 *   AUTH_GATEWAY_PORT — Auth Gateway 端口（默认 19000）
 */

'use strict';

const http = require('http');

// ── 配置 ────────────────────────────────────────────────────────────────────

const PROXY_PORT = process.env.AUTH_GATEWAY_PORT || '19000';
const PROXY_HOST = '127.0.0.1';
const API_PATH = '/proxy/prosearch/search';
const REQUEST_TIMEOUT = 10000; // 10 秒超时

// ── 参数解析 ─────────────────────────────────────────────────────────────────

const rawArg = process.argv[2];

if (!rawArg) {
  const errorResult = {
    success: false,
    message: '缺少搜索参数。用法: node prosearch.js \'{"keyword":"搜索关键词"}\''
  };
  console.log(JSON.stringify(errorResult));
  process.exit(1);
}

let params;
try {
  params = JSON.parse(rawArg);
} catch (e) {
  const errorResult = {
    success: false,
    message: `JSON 参数解析失败: ${e.message}`
  };
  console.log(JSON.stringify(errorResult));
  process.exit(1);
}

if (!params.keyword) {
  const errorResult = {
    success: false,
    message: '缺少必填参数 keyword。'
  };
  console.log(JSON.stringify(errorResult));
  process.exit(1);
}

// ── 构建请求体（只保留有效参数）──────────────────────────────────────────────

const body = {};
body.keyword = params.keyword;
if (params.mode !== undefined) body.mode = params.mode;
if (params.cnt !== undefined) body.cnt = params.cnt;
if (params.site !== undefined) body.site = params.site;
if (params.from_time !== undefined) body.from_time = params.from_time;
if (params.to_time !== undefined) body.to_time = params.to_time;
if (params.industry !== undefined) body.industry = params.industry;

const requestBody = JSON.stringify(body);

// ── 发送请求 ─────────────────────────────────────────────────────────────────

const req = http.request(
  {
    host: PROXY_HOST,
    port: Number(PROXY_PORT),
    path: API_PATH,
    method: 'POST',
    timeout: REQUEST_TIMEOUT,
    headers: {
      'Content-Type': 'application/json',
      'Content-Length': Buffer.byteLength(requestBody),
    },
  },
  (res) => {
    let data = '';
    res.setEncoding('utf8'); // 关键：强制 UTF-8 解码，避免编码问题
    res.on('data', (chunk) => {
      data += chunk;
    });
    res.on('end', () => {
      // 直接输出响应 JSON
      console.log(data);
    });
  }
);

req.on('timeout', () => {
  req.destroy();
  const errorResult = {
    success: false,
    message: `搜索请求超时（${REQUEST_TIMEOUT / 1000}秒）。请稍后重试。`
  };
  console.log(JSON.stringify(errorResult));
  process.exit(1);
});

req.on('error', (err) => {
  const errorResult = {
    success: false,
    message: `搜索请求失败: ${err.message}`
  };
  console.log(JSON.stringify(errorResult));
  process.exit(1);
});

req.write(requestBody);
req.end();
