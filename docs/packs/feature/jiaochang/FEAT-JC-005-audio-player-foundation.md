# FEAT-JC-005: Audio Player Foundation

## Status
- State: done
- Completed: 2026-04-26

## Goal
在校场 cockpit 中加入不自动播放的音乐播放器基础层。首版只做 bundled/local/cache 与 adapter 合同，保留 CeruMusic 式“metadata 不等于 playable URL”的播放前解析策略，并为 FEAT-JC-006 插件音源沙箱留口。

## Spec (verifiable)
- `src/modules/jiaochang/audio/**` 分层包含 provider / hook / state / events / visualizer / track-library
- `resolveTrackUrl` 是播放前唯一 URL 解析入口，track metadata 不直接等于 playable URL
- bundled/local/cache adapters 可工作；service/plugin adapter 默认禁用并返回证据化错误
- 本地音乐导入通过 Tauri dialog 选择文件，记录 `permissionRef` grant，并持久化 local track metadata
- 播放器不自动播放；无曲目时显示明确空状态
- UI 不散落外部 URL、插件解析、缓存逻辑；组件只调用 provider controls
- 新文案覆盖 zh-CN/en-US/ja-JP/ko-KR

## Files (scope)
- `src/modules/jiaochang/audio/**`
- `src/modules/jiaochang/components/JiaochangMusicPlayer.tsx`
- `src/modules/jiaochang/JiaochangPage.tsx`
- `src/modules/jiaochang/i18n/**`
- `docs/packs/feature/jiaochang/FEAT-JC-005-audio-player-foundation.md`
- `docs/packs/REGISTRY.md`

## Reads
- `docs/product-specs/jiaochang-pixel-agent-board-prd.md` FEAT-JC-005 / CeruMusic 参考章节
- [CeruMusic](https://github.com/timeshiftsauce/CeruMusic) `ControlAudio`, `musicCache`, plugin URL resolution strategy
- [Star Office UI](https://github.com/ringhyacinth/Star-Office-UI) only for pixel cockpit interaction direction

## Contract (review must check)
- 不内置第三方曲库，不破解或绕过授权
- 不执行 renderer 插件代码；plugin/service adapters 只预留，FEAT-JC-006 再做 Rust side isolate / worker
- 不自动播放
- 不新增 raw Tauri event subscription
- 不复制 CeruMusic Electron/Vue/Pinia 代码，只复刻策略与边界
- 不把 runtime truth 或 mini mode 做成第二套数据源

## Out of Scope
- 插件音源实现
- Rust side isolate / worker
- 远程 service 音源 catalog
- OS 级 security-scoped bookmark / 后端授权校验
- 歌词、歌单编辑、波形分析器真实 FFT

## Verify
- `node --experimental-strip-types --import='data:text/javascript,import {register} from "node:module"; import {pathToFileURL} from "node:url"; register("./scripts/test-loader.mjs", pathToFileURL("./"))' --test 'src/modules/jiaochang/**/*.test.ts'`
- `npm run build:web`

## Done
- 上面 verify PASS
- resolver/cache/events 有测试覆盖
- REGISTRY 状态改为 done
