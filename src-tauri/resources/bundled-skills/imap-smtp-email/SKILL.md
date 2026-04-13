---
name: imap-smtp-email
description: 通过 IMAP/SMTP 连接个人邮箱，支持完整邮件收发、抄送、附件、HTML、收件箱检索与附件下载；是当前邮件体系中唯一的个人邮箱主通道。
metadata:
  openclaw:
    emoji: "📧"
    requires:
      bins:
        - node
        - npm
    primaryEnv: SMTP_PASS
---

# IMAP / SMTP Email（个人邮箱主 Skill）

> **定位**：这是唯一的个人邮箱主 skill。只要需求不再是"推送到我自己的邮箱做留存"，而是要像正常邮箱一样完整收发邮件，就应该走 `imap-smtp-email`。
>
> **调用方式**：直接调用本 skill 的入口脚本 `<SCRIPT_PATH>/scripts/unix/email_gateway.sh`，其中 `<SCRIPT_PATH>` 指本 skill（`imap-smtp-email`）的根目录。

## 1. 现在的职责

`imap-smtp-email` 负责所有**完整邮件能力**：

- 发给任意收件人
- `to` / `cc` / `bcc`
- 纯文本 / HTML
- 附件发送
- IMAP 收件、搜索、查看详情
- 下载附件
- 标记已读 / 未读
- 列出邮箱文件夹

> 当前邮件体系中，所有个人邮箱能力都统一收敛到本 skill，不再按邮箱厂商拆分入口。

## 2. 与平台公邮的边界

| 问题 | 平台公邮 | `imap-smtp-email` |
|------|----------|-------------------|
| 推送到自己的邮箱 | ✅ | ✅ |
| 发给别人 | ❌ | ✅ |
| 抄送 / 密送 | ❌ | ✅ |
| HTML | ❌ | ✅ |
| 附件 | ❌ | ✅ |
| 收件 / 搜索 / 下载附件 | ❌ | ✅ |
| 零配置 | ✅ | ❌ |

**判断口诀：**

- **只给自己做留存**：更适合平台公邮
- **像正常邮箱那样收发**：直接 `imap-smtp-email`

> 这里的关键不是"先检查平台公邮"，而是**先理解场景**：如果任务本质是结果留存，就选平台公邮；如果任务本质是完整邮件动作，就直接选本 skill。

## 3. 收敛后的能力组成

本 skill 统一承接了个人邮箱场景里仍然有效的能力与预设：

- 网易系邮箱 provider 预设
- QQ / Foxmail / 企业邮 provider 预设
- 统一的 `get-token.sh` / `get-token.ps1` 凭证导入脚本
- 个人邮箱的统一 `.env` 配置入口

这意味着：

- `email-skill` 的个人邮箱分流目标只剩一个：`imap-smtp-email`
- 个人邮箱侧的脚本、配置和帮助信息都应围绕本 skill 维护

## 4. 支持的邮箱 Provider 预设

以下 provider 已内置到 `setup.sh` 配置向导中：

| Provider | IMAP Host | IMAP Port | SMTP Host | SMTP Port |
|----------|-----------|-----------|-----------|-----------|
| 163.com | imap.163.com | 993 | smtp.163.com | 465 |
| vip.163.com | imap.vip.163.com | 993 | smtp.vip.163.com | 465 |
| 126.com | imap.126.com | 993 | smtp.126.com | 465 |
| vip.126.com | imap.vip.126.com | 993 | smtp.vip.126.com | 465 |
| 188.com | imap.188.com | 993 | smtp.188.com | 465 |
| vip.188.com | imap.vip.188.com | 993 | smtp.vip.188.com | 465 |
| yeah.net | imap.yeah.net | 993 | smtp.yeah.net | 465 |
| gmail.com | imap.gmail.com | 993 | smtp.gmail.com | 587 |
| Outlook.com | outlook.office365.com | 993 | smtp-mail.outlook.com | 587 |
| qq.com | imap.qq.com | 993 | smtp.qq.com | 465 |
| foxmail.com | imap.qq.com | 993 | smtp.qq.com | 465 |
| yahoo.com | imap.mail.yahoo.com | 993 | smtp.mail.yahoo.com | 465 |
| sina.com | imap.sina.com | 993 | smtp.sina.com | 465 |
| sohu.com | imap.sohu.com | 993 | smtp.sohu.com | 465 |
| 139.com | imap.139.com | 993 | smtp.139.com | 465 |
| exmail.qq.com | imap.exmail.qq.com | 993 | smtp.exmail.qq.com | 465 |
| aliyun.com | imap.aliyun.com | 993 | smtp.aliyun.com | 465 |
| Custom | 自定义 | 自定义 | 自定义 | 自定义 |

> 对于 `587` 端口，`SMTP_SECURE=false`，走 STARTTLS。
> 对于 `465` 端口，`SMTP_SECURE=true`，走 SSL。

## 5. 配置方式

### 5.0 自动账号解析（推荐）

调用本 skill 的入口脚本时，会自动处理账号选择和凭证配置：

1. 调 4230 接口查询已绑定的所有个人邮箱
2. 根据 `--account-email` 参数或自动选择决定使用哪个邮箱
3. 自动调 `get-token.sh --platform xxx` 刷新凭证并写入 `.env`
4. 然后执行后续的 smtp / imap 命令

```bash
# 发送邮件
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' send \
  --account-email 'your@163.com' \
  --to 'recipient@example.com' \
  --subject 'Hello' \
  --body 'World'

# 查看收件箱
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-check \
  --account-email 'your@163.com'
```

### 5.1 配置向导（推荐）

```bash
cd '<SKILL_DIR>' && bash setup.sh
```

向导会依次询问：

- 邮箱服务商
- 邮箱地址
- 授权码 / App Password / SMTP 密码
- 是否允许自签名证书
- 允许读取附件的目录白名单
- 允许保存附件的目录白名单

### 5.2 凭证导入脚本

如果你的邮箱授权信息来自集成面板或外部流程，可以直接使用：

```bash
# 手动指定 token 和邮箱
bash get-token.sh --token '<AUTH_TOKEN>' --email 'your@qq.com'

# 指定 platform 从凭证服务拉取（支持 163_mail, qq_mail, gmail, outlook, sina_mail, sohu_mail）
bash get-token.sh --platform 163_mail

# 自动遍历所有平台（在已接通网关的情况下）
bash get-token.sh
```

Windows 下可使用 `get-token.ps1`。

### 5.3 手动写 `.env`

```bash
IMAP_HOST=imap.gmail.com
IMAP_PORT=993
IMAP_USER=your@gmail.com
IMAP_PASS=your_app_password
IMAP_TLS=true
IMAP_REJECT_UNAUTHORIZED=true
IMAP_MAILBOX=INBOX

SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_SECURE=false
SMTP_USER=your@gmail.com
SMTP_PASS=your_app_password
SMTP_FROM=your@gmail.com
SMTP_REJECT_UNAUTHORIZED=true

ALLOWED_READ_DIRS=$HOME/Downloads,$HOME/Documents
ALLOWED_WRITE_DIRS=$HOME/Downloads
```

## 6. 重要认证说明

### Gmail

- Gmail **不能**直接使用普通登录密码
- 需要开启两步验证后生成 **App Password**
- 将 App Password 用作 `IMAP_PASS` / `SMTP_PASS`

### 网易 / QQ / 腾讯企业邮 / 新浪 / 搜狐等

- 通常需要使用 **邮箱授权码**，而不是登录密码
- 使用前请先在邮箱网页端开启 IMAP / SMTP 服务

### Outlook

- 若租户策略要求，需使用 **应用专用密码** 或管理员允许的 SMTP 凭证
- 默认使用 `outlook.office365.com` + `smtp-mail.outlook.com`

## 7. IMAP 命令

> `<SCRIPT_PATH>` 指本 skill（`imap-smtp-email`）的根目录。

### `inbox-check`

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-check \
  --account-email 'your@163.com' --limit 10 --mailbox INBOX --recent 2h
```

### `inbox-search`

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-search \
  --account-email 'your@163.com' --subject 发票 --recent 7d --limit 20
```

### `inbox-fetch`

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-fetch \
  --account-email 'your@163.com' 12345 --mailbox INBOX
```

### `inbox-download`

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-download \
  --account-email 'your@163.com' 12345 --dir "$HOME/Downloads"
```

### 其他 IMAP 命令

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-mark-read --account-email 'your@163.com' 12345
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-mark-unread --account-email 'your@163.com' 12345
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-list-mailboxes --account-email 'your@163.com'
```

## 8. SMTP 命令

### `send`

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' send \
  --account-email 'your@163.com' \
  --to partner@example.com \
  --subject "Hello" \
  --body "World"
```

### 常见示例

#### 发送 HTML 邮件

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' send \
  --account-email 'your@163.com' \
  --to recipient@example.com \
  --subject "周报" \
  --html \
  --body "<h1>Weekly Report</h1><p>详情见正文</p>"
```

#### 发送附件

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' send \
  --account-email 'your@163.com' \
  --to recipient@example.com \
  --subject "报告" \
  --body "请查收附件" \
  --attach /Users/you/Documents/report.pdf
```

#### 抄送 / 密送

```bash
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' send \
  --account-email 'your@163.com' \
  --to a@example.com \
  --cc b@example.com \
  --bcc c@example.com \
  --subject "项目同步" \
  --body "请查收"
```

## 9. 与其他 skill 的关系

- `email-skill`：统一入口，负责意图识别与路由分发
- `public-skill`：平台公邮，仅做"推送到自己邮箱"
- `imap-smtp-email`（本 skill）：完整个人邮箱能力

## 10. 调用规范

**通过本 skill 的入口脚本 `<SCRIPT_PATH>/scripts/unix/email_gateway.sh` 调用，禁止直接调用内部的 `resolve-account.cjs`、`smtp.js`、`imap.js` 等脚本。**

```bash
# ✅ 正确：通过本 skill 入口脚本
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' send \
  --account-email 'your@gmail.com' \
  --to 'recipient@example.com' \
  --subject '正式邮件' \
  --body '你好'

# ✅ 正确：查收件箱
bash '<SCRIPT_PATH>/scripts/unix/email_gateway.sh' inbox-check \
  --account-email 'your@gmail.com'

# ❌ 错误：不允许直接调用内部脚本
# node scripts/resolve-account.cjs send ...
# node scripts/smtp.js send ...
# node scripts/imap.js check ...
```
