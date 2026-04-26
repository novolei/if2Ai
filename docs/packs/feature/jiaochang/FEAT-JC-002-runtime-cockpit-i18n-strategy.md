# FEAT-JC-002: Runtime Cockpit Adapter + I18n + Strategy

## Status
- State: done
- Completed: 2026-04-25

## Goal
在 FEAT-JC-001 入口骨架上补齐 runtime cockpit 的产品可信度：projection-ready adapter、四语 i18n catalog、证据化策略建议。仍不接真实后端、不做音乐/生图/窗口模式。

## Spec (verifiable)
- `src/modules/jiaochang/data/**` 支持 fixture 与 projection snapshot 同形 view model
- UI 文案集中走 `src/modules/jiaochang/i18n/**`，覆盖 zh-CN/en-US/ja-JP/ko-KR
- `blocked` / `done` / long-running / running 状态能生成 evidence-backed strategy item
- 页面展示 adapter health、tool ledger、run progress、strategy panel
- 测试覆盖 projection snapshot 映射、四语 key 完整性、blocked/done strategy

## Files (scope)
- `src/modules/jiaochang/**`
- `docs/packs/feature/jiaochang/FEAT-JC-002-runtime-cockpit-i18n-strategy.md`
- `docs/packs/REGISTRY.md`

## Reads
- `docs/product-specs/jiaochang-pixel-agent-board-prd.md` §§ 6, 8, 9, 15

## Contract (review must check)
- UI 不直接订阅 raw Tauri events
- 不新增 IPC / dependency
- 不把 fixture 字段写死进组件
- 四语新增 key 必须同形

## Out of Scope
- 真实 runtime projection 订阅
- 图片资产与像素字体文件
- 音乐播放器 / 插件音源
- ChatGPT Image 2 provider
- 独立窗口 / mini mode

## Verify
- `npm test -- --test-name-pattern=jiaochang`
- `npm run build:web`

## Done
- 上面 verify PASS
- 校场页面所有本片新增 UI 文案走 i18n catalog
- 2026-04-25 复核：projection-ready adapter、四语 catalog、blocked/done/running strategy、adapter health 与 source-aware badge 已补齐。
