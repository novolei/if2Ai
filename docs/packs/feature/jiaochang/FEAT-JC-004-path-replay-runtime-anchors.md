# FEAT-JC-004: Path Replay + Runtime Anchors

## Status
- State: done
- Completed: 2026-04-26

## Goal
在 FEAT-JC-001/002/003 的可信校场 cockpit 基础上，补齐 Agent 路径回放与 runtime anchor。用户可以从校场事件看到 Agent 在场景中的状态迁移路线，并通过 typed anchor 回到 Chat / tool / file 上下文。

## Spec (verifiable)
- `src/modules/jiaochang/data/**` 增加 typed runtime anchors，不把 Chat/tool/file 跳转信息写死在 UI
- fixture 至少包含 chat、tool、file 三类 anchor，并显式保持 `source: 'fixture'`
- projection snapshot 可传入事件 anchor 与 path replay steps，adapter 原样映射到 view model
- 新增 path replay typed selector，可对 fixture run 生成完整状态路径
- 页面新增路径回放面板，支持选择回放节点、上一节点、下一节点、播放/暂停
- Timeline 事件可展示 evidence anchor，并通过 callback 请求回到上下文
- UI 不直接订阅 raw Tauri events，不新增后端 IPC

## Files (scope)
- `src/modules/jiaochang/**`
- `docs/packs/feature/jiaochang/FEAT-JC-004-path-replay-runtime-anchors.md`
- `docs/packs/REGISTRY.md`

## Reads
- `docs/product-specs/jiaochang-pixel-agent-board-prd.md` §§ FEAT-JC-002/003, FR-010, FR-011
- [Star Office UI](https://github.com/ringhyacinth/Star-Office-UI)
- [CeruMusic](https://github.com/timeshiftsauce/CeruMusic) only as a reminder that future music remains adapter-first and out of this Pack

## Contract (review must check)
- 不接音乐播放器、本地音乐授权、插件音源或 AI 生图 provider
- 不复制 Star Office UI 资产或代码
- 不新增 raw Tauri event subscription
- 不把 fixture anchors 当真实 runtime 数据
- 路径回放读取 typed view model，不复制第二套 runtime truth

## Out of Scope
- 真实 runtime subscription
- 自动滚动 Chat 到具体 message / tool card 的后端命令
- 文件打开 IPC
- 区域热力图
- 音乐 / 插件 / 生图 / mini mode

## Verify
- `node --experimental-strip-types --import='data:text/javascript,import {register} from "node:module"; import {pathToFileURL} from "node:url"; register("./scripts/test-loader.mjs", pathToFileURL("./"))' --test src/modules/jiaochang/**/*.test.ts`
- `npm run build:web`

## Done
- 上面 verify PASS
- fixture run 可回放路径
- timeline 至少 3 类 fixture anchor 可请求回到上下文
