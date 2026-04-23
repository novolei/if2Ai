#!/usr/bin/env bash
# UClaw 激活服务 VPS 部署脚本（v2 — 同时支持 iClaw + UClaw）
#
# 功能：
#   - 同步本目录源码到 VPS
#   - 在 VPS 上 cargo build --release
#   - 替换二进制并重启 iclaw-activation systemd 服务
#   - 确保 .env 中 APP_IDS 同时包含 com.wt.iClaw、ai.if2.UClaw、ai.if2.if2Ai（与 UClaw 多 app 写法一致）
#
# VPS 目录结构：
#   /opt/iclaw-activation-server/src/      ← 项目根 (Cargo.toml)
#   /opt/iclaw-activation-server/src/src/  ← Rust 源码 (main.rs)
#   /opt/iclaw-activation-server/bin/      ← 二进制输出
#   /opt/iclaw-activation-server/.env      ← 环境变量
#
# 用法:
#   ./deploy-to-vps.sh [VPS_HOST]
#   export ICLAW_VPS_HOST=root@your-vps-ip && ./deploy-to-vps.sh
#   echo "root@your-vps-ip" > .deploy-host && ./deploy-to-vps.sh
#
# 可选环境变量:
#   ICLAW_VPS_HOST      VPS 登录地址，如 root@1.2.3.4
#   ICLAW_VPS_PASSWORD  SSH 密码（不设置则走 SSH key）

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# ── 解析 VPS 主机 ──────────────────────────────────────────────
VPS_HOST="${1:-${ICLAW_VPS_HOST:-}}"
if [[ -z "$VPS_HOST" && -f ".deploy-host" ]]; then
    VPS_HOST="$(grep -v '^#' .deploy-host | grep -v '^[[:space:]]*$' | head -1)"
fi

if [[ -z "$VPS_HOST" || "$VPS_HOST" == *"your-vps"* || "$VPS_HOST" == *"REPLACE"* ]]; then
    echo ""
    echo "❌ 未配置 VPS 主机，无法部署。"
    echo ""
    echo "配置方式（任选其一）："
    echo "  1. 环境变量: export ICLAW_VPS_HOST=root@你的VPS-IP"
    echo "  2. 命令行:   $0 root@你的VPS-IP"
    echo "  3. 配置文件: echo 'root@你的VPS-IP' > $(dirname "$0")/scripts/activation-server-rs/.deploy-host"
    exit 1
fi

BIN_NAME="iclaw-activation-server"
REMOTE_DIR="/opt/iclaw-activation-server"
REMOTE_SRC_DIR="$REMOTE_DIR/src"
REMOTE_BIN_DIR="$REMOTE_DIR/bin"

# ── SSH / rsync 包装器 ─────────────────────────────────────────
run_ssh() {
    if [[ -n "${ICLAW_VPS_PASSWORD:-}" ]] && command -v sshpass &>/dev/null; then
        sshpass -p "$ICLAW_VPS_PASSWORD" ssh -o StrictHostKeyChecking=accept-new "$VPS_HOST" "$@"
    else
        ssh -o StrictHostKeyChecking=accept-new "$VPS_HOST" "$@"
    fi
}

run_rsync() {
    if [[ -n "${ICLAW_VPS_PASSWORD:-}" ]] && command -v sshpass &>/dev/null; then
        SSHPASS="$ICLAW_VPS_PASSWORD" sshpass -e rsync "$@"
    else
        rsync "$@"
    fi
}

echo "🚀 UClaw 激活服务部署 → $VPS_HOST"
echo ""

# ── 1/5 同步源码 ───────────────────────────────────────────────
echo "==> 1/5 同步源码到 $VPS_HOST:$REMOTE_SRC_DIR ..."
run_rsync -avz --delete \
    -e "ssh -o StrictHostKeyChecking=accept-new" \
    --exclude 'target' --exclude '.git' --exclude '.deploy-host' --exclude '.env' \
    "$SCRIPT_DIR/" "$VPS_HOST:$REMOTE_SRC_DIR/"

# ── 2/5 在 VPS 上编译 ─────────────────────────────────────────
echo ""
echo "==> 2/5 在 VPS 上 cargo build --release ..."
run_ssh "export PATH=/root/.cargo/bin:\$PATH && cd $REMOTE_SRC_DIR && cargo build --release 2>&1"

# ── 3/5 替换二进制 ─────────────────────────────────────────────
echo ""
echo "==> 3/5 替换二进制..."
run_ssh "mkdir -p $REMOTE_BIN_DIR && systemctl stop iclaw-activation 2>/dev/null || true && cp $REMOTE_SRC_DIR/target/release/$BIN_NAME $REMOTE_BIN_DIR/$BIN_NAME"

# ── 4/5 更新 .env — 确保 APP_IDS 同时包含 iClaw、UClaw、if2Ai ─────
echo ""
echo "==> 4/5 更新 .env: 确保 APP_IDS=com.wt.iClaw,ai.if2.UClaw,ai.if2.if2Ai ..."
run_ssh "
set -e
ENV_FILE=$REMOTE_DIR/.env
REQUIRED_APP_IDS='com.wt.iClaw,ai.if2.UClaw,ai.if2.if2Ai'

if [ ! -f \"\$ENV_FILE\" ]; then
    echo \"# iClaw / UClaw / if2Ai Activation Server env\" > \"\$ENV_FILE\"
fi

# 移除旧的 APP_ID / APP_IDS 行，统一写入 APP_IDS（与 UClaw 多客户端白名单一致）
sed -i.bak '/^APP_ID=/d; /^APP_IDS=/d' \"\$ENV_FILE\"
echo \"APP_IDS=\$REQUIRED_APP_IDS\" >> \"\$ENV_FILE\"
echo \"✅ .env APP_IDS 已设置为: \$REQUIRED_APP_IDS\"
grep '^APP_IDS=' \"\$ENV_FILE\" || true
"

# ── 5/5 重启服务并检查状态 ─────────────────────────────────────
echo ""
echo "==> 5/5 重启 iclaw-activation 服务..."
run_ssh "systemctl start iclaw-activation && sleep 2 && systemctl status iclaw-activation --no-pager" || true

echo ""
echo "✅ 部署完成。"
echo ""
echo "验证激活服务（三个 app_id 均应通过白名单校验）:"
echo "  iClaw:  curl -s https://console.iclaw.us/license-api/healthz"
echo "  UClaw:  curl -s -X POST https://console.iclaw.us/license-api/v1/activations/request \\"
echo "          -H 'Content-Type: application/json' \\"
echo "          -d '{\"installation_id\":\"test\",\"app_id\":\"ai.if2.UClaw\",\"app_version\":\"1.0\",\"build\":\"1\",\"platform\":\"macOS\"}'"
echo "  if2Ai:  curl -s -X POST https://console.iclaw.us/license-api/v1/activations/request \\"
echo "          -H 'Content-Type: application/json' \\"
echo "          -d '{\"installation_id\":\"test\",\"app_id\":\"ai.if2.if2Ai\",\"app_version\":\"0.3.0\",\"build\":\"1\",\"platform\":\"macOS\"}'"
