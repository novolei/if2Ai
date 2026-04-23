# iClaw Activation Server (Rust)

轻量激活服务，提供：
- 设备申请与审批激活
- License JWS（Ed25519）签发
- Refresh 续期与 token rotation
- 吊销检查
- 简易 Web 管理台（Basic Auth）

## 主要接口

- `GET /healthz`
- `POST /v1/activations/request`
- `GET /v1/activations/request/:id`
- `POST /v1/activations/redeem`
- `POST /v1/activations/redeem-by-code`（8 位邀请码兑换）
- `POST /v1/licenses/refresh`
- `POST /v1/licenses/revoke-check`
- `GET /admin`

## 多应用 `app_id`（与 UClaw 一致）

- 环境变量 **`APP_IDS`**：逗号分隔白名单，例如  
  `com.wt.iClaw,ai.if2.UClaw,ai.if2.if2Ai`  
  **if2Ai** 客户端固定使用 `ai.if2.if2Ai`（与 if2Ai 仓库 `LicenseLifecycleService::APP_ID` 对齐）。
- 若未设置 `APP_IDS`，则回退读取 **`APP_ID`**（单值）；再未设置则使用代码内默认白名单（含上述三者）。

## 导出 if2Ai 客户端验签公钥

if2Ai 在构建时嵌入 **Ed25519 公钥**（验 `license_jws`）。在已配置
`SIGNING_KEY_B64` 的环境执行：

```bash
export SIGNING_KEY_B64='…'   # 与 .env 中一致
cargo run --example print_verifying_key
```

将打印的一行写入 if2Ai 的  
`src-tauri/keys/activation_license_ed25519_pub.b64`，或在构建 if2Ai 时设置  
`IF2AI_LICENSE_ED25519_PUBKEY_B64`（见该 crate 的 `build.rs`）。

## 本地运行

1. 复制环境变量：
   - `cp .env.example .env`
2. 安装 Rust 后运行：
   - `cargo run --release`

## VPS 部署（systemd）

推荐目录：
- 代码：`/opt/iclaw-activation-server/src`
- 二进制：`/opt/iclaw-activation-server/bin/iclaw-activation-server`
- 配置：`/opt/iclaw-activation-server/.env`
- 服务文件：`/etc/systemd/system/iclaw-activation.service`

启动：
- `systemctl daemon-reload`
- `systemctl enable --now iclaw-activation.service`

默认监听地址由 **`BIND_ADDR`** 决定（示例为 `127.0.0.1:78789`；生产常见为 `127.0.0.1:58789`），由 FastPanel Nginx 反代到公网（例如 `https://console.iclaw.us/license-api/` → 本机 `/`）。
