# FEAT-JC-001: Jiaochang Shell + Fixture Cockpit

## Status
- State: done
- Completed: 2026-04-25

## Goal
新增左侧 `校场` 一级入口，点击进入真实页面骨架。首片只实现可信 runtime cockpit 的 typed fixture 投影，不接音乐、插件、生图或真实后端。

## Spec (verifiable)
- `AppSection` 支持 `jiaochang`，GlobalNavbar 显示 `校场` 导航入口
- SectionWorkspace 对 `jiaochang` 渲染真实 `JiaochangPage`，不是占位空态
- `src/modules/jiaochang/data/**` 暴露 typed agents / tool ledger / run progress fixture view model
- 页面展示 pixel-stage、运行面板、时间线、fixture badge 和返回 Chat 操作
- fixture selector 测试覆盖 subagent identity、tool ledger、run progress

## Files (scope)
- `src/modules/app-shell/types.ts`
- `src/modules/app-shell/components/GlobalNavbar.tsx`
- `src/modules/app-shell/components/SectionWorkspace.tsx`
- `src/App.tsx`
- `src/modules/jiaochang/**` (new)
- `docs/packs/REGISTRY.md`

## Reads
- `docs/product-specs/jiaochang-pixel-agent-board-prd.md` §§ 1-8, 15

## Contract (review must check)
- UI 不直接订阅 raw Tauri events
- 不新增 IPC / dependency / 音乐 / 生图 / 插件 sandbox
- fixture 必须明确标注，组件只消费 typed view model
- 不改 ChatWorkspace / MemoryBrowser 行为

## Out of Scope
- 真实 runtime projection 接入
- 图片资产生成与像素字体落地
- 音乐播放器、本地授权、插件音源
- ChatGPT Image 2 provider 配置
- 独立窗口 / mini mode

## Verify
- `npm test -- --test-name-pattern=jiaochang`
- `npm run build:web`

## Done
- 上面 verify PASS
- 左侧 `校场` 可进入 fixture cockpit 页面
- 2026-04-25 复核：入口、`SectionWorkspace` 路由、typed fixture selector、fixture badge、返回 Chat 操作均已按代码事实完成。
