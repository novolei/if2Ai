# GFR-T2-A: Split `ProviderSetupStep.tsx` (1764 LOC)

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-T1-I（与后端解耦，可早期开始）` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 T2-A 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src/modules/onboarding/steps/ProviderSetupStep.tsx`
-   - provider 卡片选择（国内 / 国际 两 section）
-   - 配置表单（API key / endpoint / 等）
-   - Ollama 自动连接 + 模型多选
-   - 动画 keyframes 注入
-   - 模型列表搜索

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- `onboarding/steps/provider/`：`ProviderSetupStep.tsx`（layout shell ≤ 200 行）
-   - `provider-cards/ProviderCardGrid.tsx`
-   - `config-form/ProviderConfigForm.tsx`（按 provider kind 分支）
-   - `model-selector/ModelMultiSelect.tsx` + `ollama-auto-connect.ts`
-   - `animations.ts`（keyframes 注入抽出）

---

## Files (scope)

- src/modules/onboarding/steps/ProviderSetupStep.tsx

---

## Out of Scope

- ❌ 不改 onboarding step 顺序
- ❌ 不改 provider 分组规则（国内/国际）
- ❌ 不动写入 `OnboardingState` 的字段

---

## Activation Trigger

1. `GFR-T1-I（与后端解耦，可早期开始）` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
