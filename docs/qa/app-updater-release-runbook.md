# App Updater Release Runbook

日期: 2026-04-25

## 目标

把 If2Ai macOS 构建产物、Tauri 签名 updater artifact、`latest.json` 和 `if2ai-release-manifest.json` 上传到 GitHub Release，使客户端能自动发现、下载、签名校验并安装新版本。

## 签名密钥

首次接入需要生成 Tauri updater signing key，并把私钥写入 GitHub Secrets:

```bash
npm run tauri signer generate -- -w ~/.tauri/if2ai-updater.key
```

Secrets:

- `TAURI_SIGNING_PRIVATE_KEY`: 私钥内容或私钥路径内容。
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: 私钥密码；无密码时设为空字符串。
- `TAURI_SIGNING_PUBLIC_KEY`: 公钥内容；运行时也可用 `IF2AI_UPDATER_PUBKEY` 覆盖。

## 发布命令

```bash
npm run release:macos -- patch
npm run release:github -- --repo=novolei/if2Ai --channel=stable
```

一条命令完成打包 + 上传:

```bash
npm run release:github -- --build=patch --repo=novolei/if2Ai --channel=stable
```

常用参数:

- `--version=0.4.1`: 覆盖 `package.json` 中的版本号。
- `--tag=v0.4.1`: 覆盖 GitHub Release tag。
- `--artifact=/absolute/path/If2Ai_0.4.1_aarch64.dmg`: 指定上传产物。
- `--updater-artifact=/absolute/path/If2Ai.app.tar.gz`: 指定 Tauri updater bundle。
- `--signature=/absolute/path/If2Ai.app.tar.gz.sig`: 指定 updater signature。
- `--out-dir=/absolute/path/to/release-dir`: 指定 `release:macos` 输出目录。
- `--dry-run`: 只生成并校验 manifest，不调用 `gh release`。
- `--build=patch`: 先调用 `scripts/release-macos.sh patch`，再生成 manifest 并上传。

## 产物

脚本会生成:

- `dist/release/if2ai-release-manifest.json`
- `dist/release/latest.json`
- GitHub Release artifact，例如 `If2Ai_0.4.1_aarch64.dmg`
- Tauri updater artifact，例如 `If2Ai.app.tar.gz`
- Tauri signature，例如 `If2Ai.app.tar.gz.sig`

manifest URL 约定:

```text
https://github.com/novolei/if2Ai/releases/latest/download/if2ai-release-manifest.json
https://github.com/novolei/if2Ai/releases/latest/download/latest.json
```

客户端默认读取这个 URL；也可以用环境变量覆盖:

```bash
IF2AI_UPDATE_MANIFEST_URL=https://example.com/if2ai-release-manifest.json
```

## 客户端行为

- 启动后最多每 6 小时后台检查一次 manifest。
- 设置页 `关于 > 软件更新` 可手动检查。
- 发现更新后，用户可点击“下载并安装”。
- 客户端主安装路径走 Tauri updater：下载 updater bundle、验证 `.sig`、安装并退出/重启。
- legacy `if2ai-release-manifest.json` 下载打开路径仅作为兼容/诊断 fallback。

## 边界

macOS 桌面 App 不能安全地在运行中无感替换自身二进制。当前实现是“热通知 + 热下载 + 签名校验 + 安装器接管”；安装过程可能会退出当前 App，安装完成后运行新版本。
