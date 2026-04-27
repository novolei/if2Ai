# FEAT-JC-010: LX/Ceru Real Resolve Worker

Status: done
Completed: 2026-04-27

## Goal

让星海 / 六音这类 LX/Ceru 兼容插件不再只停留在静态检查，而是可以在 renderer 之外的隔离 worker 中执行 `musicUrl` resolve；同时把播放失败提示改得可诊断，区分授权 resolver 未运行、resolver 返回 HTTP 错误、resolver 返回 JSON/HTML 而不是音频资源。

## Spec

- 设置页支持把本地 LX/Ceru JS 插件注册为 `lx_ceru_js` manifest，并持久化 `local_js_path` / SHA256 / source URL / provider 列表。
- 后端 `jiaochang_audio_plugin_import_lx_ceru_js_file` 从本地 JS 生成 manifest，不在 renderer 执行第三方 JS。
- `resolveTrackUrl` 对 `lx_ceru_js` manifest 走 Rust-side worker：
  - child worker 用 Node VM shim 执行插件脚本。
  - 注入最小 `globalThis.lx` API：`EVENT_NAMES` / `currentScriptInfo` / `request` / `on` / `send`。
  - 调用插件注册的 request handler，传入 `action=musicUrl`、provider source、quality 和 musicInfo。
  - worker 超时、无 handler、无 URL、非 http(s) URL 均 fail closed。
- 星海插件 provider 代码按 LX 生态保留：`kw` / `kg` / `tx` / `wy` / `mg`。
- 六音插件走同一隔离 worker 路径；如果混淆/反调试导致 worker 超时，需要明确返回 isolate timeout，而不是在 UI 中静默失败。
- 播放前对本地 resolver / provider / LX 解析 URL 做 `HEAD` 探测：
  - 连接失败或超时：提示“授权 resolver 未运行或不可访问”。
  - HTTP 非 2xx：提示 resolver 返回 HTTP 状态码。
  - Content-Type 非音频：提示“返回的不是音频资源”。

## Compliance

- 不内置第三方曲库、平台 token、cookie、VIP 绕过、DRM 绕过。
- 不在 renderer 直接执行插件代码。
- JS worker 是兼容层，不代表对任何第三方服务授权；用户仍需自行确认插件来源、平台授权和使用边界。

## Files

- `src-tauri/src/modules/jiaochang_audio/mod.rs`
- `src-tauri/src/commands/jiaochang_audio.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/command_surface.rs`
- `src/modules/jiaochang/audio/plugin-source-adapter.ts`
- `src/modules/jiaochang/audio/plugin-source-settings.ts`
- `src/modules/jiaochang/audio/track-library.ts`
- `src/modules/jiaochang/audio/JiaochangAudioProvider.tsx`
- `src/modules/settings/pages/JiaochangAudioSettingsPage.tsx`

## Out Of Scope

- 不实现完整 QuickJS / Deno sidecar。
- 不保证所有混淆 LX 插件都能运行；反调试或非标准插件应明确失败。
- 不提供平台账号授权、cookie 管理或第三方曲库搜索真实 API。

## Verify

- `cargo test -p if2ai-backend modules::jiaochang_audio --lib`
- `node --experimental-strip-types --import='data:text/javascript,import {register} from "node:module"; import {pathToFileURL} from "node:url"; register("./scripts/test-loader.mjs", pathToFileURL("./"))' --test 'src/modules/jiaochang/**/*.test.ts'`
- `npm run build:web`

## Result

- Rust worker 增加 mock LX resolve 覆盖，确认 URL 来自隔离 worker。
- Frontend plugin adapter 覆盖 LX/Ceru import command。
- Playback error copy 增加 resolver / HTTP / non-audio 三类诊断。
