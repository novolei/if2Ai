# Identity Foundation Pack Set

## Status

- State: `active`
- Owner: `@executor`
- Last Updated: `2026-04-22`

---

## Goal

为 If2Ai 引入完整的 `Soul / Persona / ResolvedIdentity` 基础层，打通：

- identity domain model
- prompt planner block 注入
- session 持久化与 API surface
- Settings / Session UI 切换
- memory identity tagging

---

## Sequence

按以下顺序执行：

1. [FEAT-ID-001](./FEAT-ID-001-identity-domain-and-resolution.md)
2. [FEAT-ID-002](./FEAT-ID-002-prompt-planner-soul-persona-blocks.md)
3. [FEAT-ID-003](./FEAT-ID-003-session-persistence-and-identity-commands.md)
4. [FEAT-ID-004](./FEAT-ID-004-settings-and-session-identity-ui.md)
5. [FEAT-ID-005](./FEAT-ID-005-memory-identity-tagging-and-observability.md)

---

## Shared Contract

- Persona 不得覆盖 Soul 的核心原则与 non-negotiables
- Identity 只影响 prompt、session metadata、memory tagging 与 UI 表达
- Identity 首版不得直接改变 permission mode / sandbox / tool allowlist
- Legacy session / memory data 必须兼容读取
- Pack 之间不得跳步；后续 pack 依赖前序 pack 的 contract 成立

---

## Source Of Truth

- [Identity Design Doc](../../../design-docs/identity-soul-persona-memory-foundation.md)

---

## Out Of Scope For This Pack Set

- 用户自定义 Soul / Persona 编辑器
- project-level identity defaults
- request-level identity override
- identity-aware retrieval ranking
- reflection / compaction / consolidation 深改
