# FEAT-JC-009: LX/Ceru JS Plugin Compatibility Sandbox

Status: done
Completed: 2026-04-26

## Goal

让校场音乐插件系统开始兼容 LX/Ceru JS 音源生态，同时不破坏既有合规边界：renderer 不执行第三方 JS，不内置第三方曲库，不破解或绕过授权。首版交付静态检查、风险标注、五源授权模板和 provider-aware resolve。

## Spec

- 支持静态检查本地 LX/Ceru JS 文件，提取 `@name` / `@version` / `@homepage` / SHA256 / size。
- 静态识别高风险行为：`new Function`、`eval`、`debugger`、DOM shim、XHR/fetch、cookie/user-agent、超长混淆行。
- 设置页新增 LX/Ceru 兼容导入区，用户可输入本地 JS 插件路径并查看风险报告。
- 六音插件 `/Users/ryanliu/Downloads/music/sixyin-music-source-v1.0.7.js` 与星海插件 `/Users/ryanliu/Downloads/V260418/第三批次/xinghai-music-source2.3.0.js` 均走同一静态检查路径。
- 新增五源授权模板 `if2ai.authorized-cn-music`，包含酷我、酷狗、QQ 音乐、网易云音乐、咪咕音乐五个 provider。
- 五源模板只指向用户自管合法 resolver endpoint：`http://127.0.0.1:43179/jiaochang/music/{provider}/resolve`，不内置平台密钥、曲库、破解接口或绕授权逻辑。
- 播放器搜索时，授权多源 manifest 会展开为 provider candidates；播放前仍走 Rust worker `resolveTrackUrl`。
- 后端 worker 支持 provider-aware route，每个 provider 可有独立 `resolver_template` 和 `allowed_hosts`。
- LX/Ceru JS 插件首版只做 quarantine/inspection，不执行 JS。

## Files

- `src-tauri/src/modules/jiaochang_audio/mod.rs`
- `src-tauri/src/commands/jiaochang_audio.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/command_surface.rs`
- `src/modules/jiaochang/audio/plugin-source-adapter.ts`
- `src/modules/jiaochang/audio/plugin-source-settings.ts`
- `src/modules/jiaochang/audio/track-library.ts`
- `src/modules/jiaochang/audio/plugin-source-adapter.test.ts`
- `src/modules/jiaochang/audio/track-library.test.ts`
- `src/modules/settings/pages/JiaochangAudioSettingsPage.tsx`

## Out Of Scope

- 不在 renderer 执行 LX/Ceru JS。
- 不实现完整 JavaScript VM / QuickJS / Deno isolate。
- 不内置酷我、酷狗、QQ、网易云、咪咕真实播放 URL 抓取逻辑。
- 不提供第三方曲库、平台 token、cookie、VIP 绕过、DRM 绕过。

## Verify

- `node --experimental-strip-types --import='data:text/javascript,import {register} from "node:module"; import {pathToFileURL} from "node:url"; register("./scripts/test-loader.mjs", pathToFileURL("./"))' --test 'src/modules/jiaochang/**/*.test.ts'`
- `cargo test -p if2ai-backend modules::jiaochang_audio --lib`
- `npm run build:web`

## Result

- Jiaochang frontend tests pass: 23/23.
- Jiaochang Rust plugin tests pass: 6/6.
- Web build passes.
