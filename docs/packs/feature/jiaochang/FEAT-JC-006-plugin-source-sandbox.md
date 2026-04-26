# FEAT-JC-006: Plugin Source Sandbox

## Status
- State: done
- Completed: 2026-04-26

## Goal
把校场音乐插件音源从 renderer 中移出，落到 Rust side isolate / worker。复刻 CeruMusic 的 cache-first → plugin resolve → async cache 思路，但不复制 Electron/Vue/Pinia 代码、不内置第三方曲库、不绕过授权。

## Spec (verifiable)
- Rust 新增 `modules::jiaochang_audio`，插件 manifest 注册、URL 解析、cache 与事件都在后端
- 插件解析通过 tokio worker + oneshot reply，不在 renderer 执行插件代码
- manifest 必须声明 `allowed_hosts`，worker 拒绝越权 host 与非 http/https URL
- 服务端 cache 优先返回已解析 URL，并通过 Tauri event 发出 cache/resolved 状态
- 新增 Tauri commands：register/list/resolve/cache_info/cache_clear
- 前端 plugin source adapter 只调用 Tauri command，不散落 URL/cache/plugin 逻辑到 UI
- Provider 内部把 Tauri plugin event 转成 typed audio event bus；UI 不直接订阅 raw Tauri events

## Files (scope)
- `src-tauri/src/modules/jiaochang_audio/**`
- `src-tauri/src/commands/jiaochang_audio.rs`
- `src-tauri/src/modules/mod.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/command_surface.rs`
- `src/modules/jiaochang/audio/**`
- `docs/packs/feature/jiaochang/FEAT-JC-006-plugin-source-sandbox.md`
- `docs/packs/REGISTRY.md`

## Reads
- `docs/product-specs/jiaochang-pixel-agent-board-prd.md` FEAT-JC-006 / CeruMusic reference
- [CeruMusic](https://github.com/timeshiftsauce/CeruMusic) `getMusicUrl`, `musicCache`, plugin service IPC strategy

## Contract (review must check)
- 不执行 renderer 插件代码
- 不内置第三方曲库，不破解或绕过授权
- 不让 UI 直接订阅 raw Tauri events
- 不新增第三方依赖
- 不实现完整插件市场、下载器、歌词、搜索、远程 catalog

## Out of Scope
- JavaScript/WASM 任意代码沙箱执行
- 插件下载、签名、权限 UI
- 缓存音频字节下载
- 插件搜索/歌单/歌词 API

## Verify
- `node --experimental-strip-types --import='data:text/javascript,import {register} from "node:module"; import {pathToFileURL} from "node:url"; register("./scripts/test-loader.mjs", pathToFileURL("./"))' --test 'src/modules/jiaochang/**/*.test.ts'`
- `cargo test --manifest-path src-tauri/Cargo.toml jiaochang_audio --lib`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- 上面 targeted verify PASS
- Pack verify 只剩已知无关 provider capability 测试失败时可报告为外部 blocker
