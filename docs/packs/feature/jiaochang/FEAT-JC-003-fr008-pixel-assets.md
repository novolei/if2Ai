# FEAT-JC-003: FR-008 Pixel Asset Seed

## Status
- State: done
- Completed: 2026-04-25

## Goal
按 FR-008 先补齐当前校场页面实际用到的原创图片资产。资产严格参考 `/Users/ryanliu/Downloads/platform.png` 的白墙黑瓦、朱红木构、湖蓝水道、等距庭院与明亮像素色彩，不复制 Star Office UI 资产。

## Spec (verifiable)
- 新增 `src/assets/jiaochang/**` 默认主题资产
- PixelStage 使用打包图片资产作为主背景与 Agent sprite
- 资产命名稳定、无空格、无文字水印
- 保留 fixture/demo 标注，不把文字写进图片
- 开发期真实生成入口必须调用 ChatGPT Images 2.0 / `gpt-image-2`，无 credential 时失败，不落假资产
- 构建可正常加载资产

## Files (scope)
- `src/assets/jiaochang/**`
- `src/modules/jiaochang/components/PixelStage.tsx`
- `scripts/generate-jiaochang-fr008-assets.mjs`
- `package.json`
- `docs/packs/feature/jiaochang/FEAT-JC-003-fr008-pixel-assets.md`
- `docs/packs/REGISTRY.md`

## Reads
- `docs/product-specs/jiaochang-pixel-agent-board-prd.md` § FR-008
- `/Users/ryanliu/Downloads/platform.png`

## Contract (review must check)
- 原创资产，不复制 Star Office UI 或第三方受限素材
- 资产风格不漂移：等距、清亮、白墙黑瓦、朱红、湖蓝、庭院建筑语言
- 生成脚本固定读取 `/Users/ryanliu/Downloads/platform.png` 作为风格锚点
- 输出按 FR-008 写入 PNG/WebP：背景与封面 WebP，sprite/icon/decor PNG
- `OPENAI_API_KEY` 不存在时必须阻断，避免把占位图误认为真实资产
- 不引入新 dependency

## Generation Contract

- 命令：`npm run jiaochang:assets`
- 模型：默认 `gpt-image-2`，可用 `JIAOCHANG_IMAGE_MODEL` 覆盖。
- API Base：默认 `https://api.openai.com/v1`，可用 `OPENAI_API_BASE` 覆盖。
- 风格锚点：`/Users/ryanliu/Downloads/platform.png`
- 生成策略：用参考图作为 image input，逐项生成 FR-008 资产；背景、移动背景、封面生成 PNG 后转换为 WebP。
- 输出范围：
  - `src/assets/jiaochang/backgrounds/shendiao-courtyard.png`
  - `src/assets/jiaochang/backgrounds/shendiao-courtyard.webp`
  - `src/assets/jiaochang/backgrounds/shendiao-courtyard-mobile.png`
  - `src/assets/jiaochang/backgrounds/shendiao-courtyard-mobile.webp`
  - `src/assets/jiaochang/cover.png`
  - `src/assets/jiaochang/cover.webp`
  - `src/assets/jiaochang/sprites/*.png`
  - `src/assets/jiaochang/icons/status-*.png`
  - `src/assets/jiaochang/decor/*.png`

## Out of Scope
- app 内 AI 生图 provider 设置页
- 音乐播放器和插件音源
- 真实 runtime projection
- 独立窗口 / mini mode

## Verify
- `node --check scripts/generate-jiaochang-fr008-assets.mjs`
- `npm run jiaochang:assets` with valid `OPENAI_API_KEY`
- `npm run build:web`

## Done
- 生成器 PASS，真实 PNG/WebP 资产已落盘，校场页面使用 `src/assets/jiaochang/**` 资产
- 2026-04-25 复核：背景、封面、状态图标、装饰、idle/sleepy sprite、status/move normalized sheets 与单帧序列均已保存到 `src/assets/jiaochang/**`；PixelStage 已读取生成的状态/移动帧。
