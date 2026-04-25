# APP-UPDATER-002: Release CI + Signed Artifact

## Status
- State: active

## Goal
把 updater 发布链从普通 GitHub artifact 升级为 Tauri 官方签名 updater artifact：CI 构建 `.app.tar.gz` + `.sig`，发布 `latest.json` 和 `if2ai-release-manifest.json`，stable channel 禁止 sha256 伪签名。

## Spec (verifiable)
- Tauri bundle 开启 `bundle.createUpdaterArtifacts = true`，并配置 updater endpoint / pubkey 占位 → `src-tauri/tauri.conf.json`
- native host 注册 `tauri-plugin-updater`，运行时可通过 `IF2AI_UPDATER_PUBKEY` 覆盖公钥 → `desktop_host::attach_native_host`
- release 脚本生成两份 manifest：
  - `latest.json`：Tauri updater 静态格式，包含 `platforms[target].url/signature`
  - `if2ai-release-manifest.json`：If2Ai 可观测 manifest，包含 checksum、signature、installer_url、signature_url
- stable manifest validation 必须拒绝 `signature: sha256:<checksum>` → `scripts/validate-release-manifest.mjs`
- tag release CI 必须构建 macOS arm64/x64 签名 updater artifact 并 gate `.app.tar.gz.sig` 存在 → `.github/workflows/release.yml`

## Files (scope)
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `src-tauri/src/modules/desktop_host/builder.rs`
- `.github/workflows/release.yml`
- `scripts/publish-github-release.mjs`
- `scripts/validate-release-manifest.mjs`
- `docs/qa/app-updater-release-runbook.md`

## Contract (review must check)
- 生产安装路径必须使用 Tauri 签名校验，不接受 sha256 伪签名替代
- 私钥只通过 GitHub Secrets / 本地 env 注入，不写入仓库
- `latest.json` 的 `signature` 是 `.sig` 文件内容，不是 `.sig` URL
- 普通 installer 与 updater bundle 可同时上传，但客户端安装只信任 updater bundle

## Out of Scope
- Windows/Linux release runner 完整矩阵
- 代码签名 / notarization 凭据接入
- 自建动态 updater server

## Verify
- `node scripts/validate-release-manifest.mjs <manifest> stable`
- tag workflow `release-ci::build_signed_updater_artifacts` 产出 `.app.tar.gz` 和 `.app.tar.gz.sig`
- `cargo check --manifest-path src-tauri/Cargo.toml`

## Done
- 上面 verify PASS
- GitHub Release 上传 `latest.json`、`if2ai-release-manifest.json`、updater artifact、`.sig`、普通安装器
