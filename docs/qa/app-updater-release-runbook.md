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

推荐的一键发布流程:

```bash
npm run release:one-click -- --bump patch
```

这个命令会自动完成:

- 读取当前真实版本，并按 `patch` / `minor` / `major` / 显式版本递增。
- 同步写入 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`。
- 用本地 `~/.tauri/if2ai-updater.key` 和 `.pub` 注入 Tauri updater 签名环境。
- 运行版本一致性检查、Tauri build、manifest dry-run 校验。
- 只 stage 三个版本文件，提交 `chore(release): vX.Y.Z`，打 annotated tag，并 push branch + tag。
- tag push 会触发 GitHub Release workflow，由 CI 构建并上传 signed updater artifact。

常用一键参数:

```bash
npm run release:one-click -- --dry-run --bump patch
npm run release:one-click -- --bump minor
npm run release:one-click -- --version 0.5.0
npm run release:one-click -- --bump patch --local-upload
```

说明:

- `--dry-run` 只预览下一版本、tag 和远端 tag 占用，不改文件。
- 默认允许工作区存在其它未提交业务改动，但版本文件必须干净，脚本只会提交版本文件。
- `--local-upload` 会在 tag push 后用本地已构建产物直接上传 release；默认推荐让 GitHub Release workflow 上传。

手动分步流程仍可用:

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
