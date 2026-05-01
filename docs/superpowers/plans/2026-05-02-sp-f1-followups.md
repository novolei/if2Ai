# SP-F1 Followups

> 来源：SP-F1 mechanical cleanup（commits `000b432`–`b6bf7d1`）执行过程中由 spec/code-quality reviewer 与 final integrated reviewer 发现、但**显式排出 SP-F1 范围**的小问题。这里登记起来，让 SP-F2 / SP-A 团队顺手吃掉。
>
> 创建时间：2026-05-02

---

## F1F-1 — `src/lib/tauri.ts` 的 `invoke` re-export 与说明已无消费者

**位置**：`src/lib/tauri.ts:35-36`

```ts
// Re-export invoke for App.tsx stopAgentStream
export { invoke };
```

FIX-11（commit `9c02d8b`）把 App.tsx 的 `stop_agent_stream` 调用迁到了 `@/api/streaming::stopAgentStream`，App.tsx 不再 `import { invoke } from '@/lib/tauri'`。全仓 grep 确认：**没有任何文件**通过 `@/lib/tauri` 拿 `invoke`（`BrowserViewerPage.tsx` 等都是直接 `from '@tauri-apps/api/core'`）。

**修复**：删除 L35-36 的 re-export 与注释。

**严重度**：Minor。**优先级**：SP-F2 第一波（5 分钟改动）。

---

## F1F-2 — `src/lib/tauri.ts` 的 `RUNTIME_EVENT_CHANNEL` re-export 已无消费者

**位置**：`src/lib/tauri.ts:78`

```ts
export { RUNTIME_EVENT_CHANNEL } from "@/transport/contracts";
```

两个真实消费者 `src/api/streaming.ts:17` 和 `src/runtime-projection/runtime-projection-bridge.ts:61` 都直接 `from '@/transport/contracts'`，不走 `@/lib/tauri` 中转。

**修复**：删除该 re-export。同时检查 `StreamTokenPayload` / `MEMORY_AFTER_TURN_EVENT` / `PermissionRequestPayload` 等其它 re-export 是否同样无消费者，做一次性 batch 清理。

**严重度**：Minor。**优先级**：SP-F2，连同 F1F-1 一起做。

---

## F1F-3 — `src/App.tsx:1392-1394` 注释仍描述 retired 频道

```ts
// The bridge subscribes broadly to `agent-token` /
// `permission-request` / `memory_event`, normalises each via the
// translator family and feeds the projection store.  Existing
```

FIX-14 删除了三个 channel 常量 + retired 频道，但本注释仍说"bridge 订阅 agent-token/permission-request/memory_event"。`wireRuntimeProjectionListeners` 实际只订阅 `runtime_event`。

**修复**：把注释改写为"the bridge subscribes to the canonical `runtime_event` envelope and dispatches by `RuntimeEventType` family"。

**严重度**：Minor（文档腐烂）。**优先级**：SP-A（本来就要重写 App.tsx 这一段）。

---

## F1F-4 — 4 处 `invoke('model_list_available', ...)` 仍未走 facade

**位置**：
- `src/components/ui/chat-ui.tsx:302`
- `src/modules/chat/components/HomeScreen.tsx:75`
- `src/modules/settings/pages/ProvidersSettingsPage.tsx:285`
- `src/modules/settings/pages/ModelSettingsPage.tsx:358`

结构与 FIX-11 处理过的 `model_get_active` / `model_set_active` 完全相同，是 SP-F1 范围外漏网。

**修复**：
1. 在 `src/api/models.ts` 增加 `listAvailableModels()` 函数（参数与 backend 命令对齐）。
2. 4 处 `invoke('model_list_available', ...)` 改为 `listAvailableModels(...)`。
3. 检查这 4 个文件的 `invoke` import 是否还有其它使用；若全部消失则删除 import。

**严重度**：Minor（与 ARCHITECTURE.md §3.2 一致性问题）。**优先级**：SP-F2（约 1 个 PR，复用 FIX-11 模板）。

---

## F1F-5 — `setActiveModel` 不接受 `auth_variant`，存在 round-trip 静默丢失

**位置**：
- `src/api/models.ts::setActiveModel`
- `src-tauri/src/commands/provider.rs:171` (`model_set_active(provider_id, model_id)`)

FIX-11 fixup 给 `ActiveModel` 加了 `auth_variant?: string` 以匹配 backend `ModelSelection` 的读路径。但 backend `model_set_active` 命令本身**不接受** `auth_variant`。结果：

UI `getActiveModel()` → `auth_variant: "moonshot-cn"` → UI 修改 model → `setActiveModel(p, m)` → **`auth_variant` 字段被静默丢弃**，下次读取该字段会消失。

**修复**：
1. Backend：扩展 `model_set_active` 命令签名，新增 `auth_variant: Option<String>` 参数。
2. Backend：`ModelResolver::set_active_model` 处理变体写入。
3. Frontend：`setActiveModel(providerId, modelId, authVariant?)`，写入 wire `{ providerId, modelId, authVariant }`。
4. 调用点更新（`chat-ui.tsx`、`HomeScreen.tsx` —— 它们目前从 `provider/model` 字符串中拆分，没有 `authVariant` 信息，需要从 `getActiveModel()` 的当前值或额外 UI 选项透传）。

**严重度**：Important（数据丢失，但仅影响 multi-auth-variant 的 provider 如 Moonshot）。**优先级**：SP-F2 / 独立小 PR。**注意**：需要先 brainstorm 一下 UI 怎么暴露 variant 选择，不是纯机械改动。

---

## F1F-6 — `9c02d8b` commit subject 行 "3 raw invoke sites" 应为 "4"

**事实**：FIX-11 实际迁移了 4 处（App.tsx 2 处 + chat-ui.tsx 1 处 + HomeScreen.tsx 1 处）。Commit body 准确描述了 4 处，但 subject 行写成 "3"。

**修复**：无法修（superpowers 规则限制 `git commit --amend`，且已 review 通过）。

**影响**：仅影响未来用 `git log --grep="4 raw"` 检索时找不到该 commit。**优先级**：N/A（仅作为 commit-author 自我提醒）。

---

## F1F-7 — `chat-store.ts` "deliberately does NOT" 新增 bullet 与 `conversation-slice` `@deprecated` 注释口径差异

**事实**：FIX-17（commit `b6bf7d1`）追加的 bullet 写"no production code path should reach them"。Code-quality reviewer 指出 `conversation-slice.ts` 的 `@deprecated` 注释允许"narrow permitted use (e.g. user-message anchoring)"——口径有摩擦。

**讨论**：保留当前 banner 是合理的（chat-store 是 facade，"no production via facade" 是 SP-A 终态目标）。但若后续仍有合理 slice-only narrow use，可微调措辞为"new code must not import these from this facade; remaining slice-only use must follow the @deprecated docs on the slice itself"。

**严重度**：Nit（措辞）。**优先级**：与 SP-A "transcript single source of truth" 一起处理，那时这两段注释要么统一删除要么统一改写。

---

## 不在本 followups 范围（已交给其它子项目）

- **FIX-1 / FIX-2 / FIX-3 / FIX-4**：chat transcript 单事实源 → SP-A
- **FIX-5 / FIX-13**：supervisor 接通 → SP-B
- **FIX-7 / FIX-9**：流式日志统一 / 反向依赖 → SP-C
- **FIX-8**：god-file 拆分 → SP-D
- **FIX-6**：harness 派生 → SP-E
- **FIX-12**：commands 层瘦身 → 后续机会主义小 PR
- **FIX-15 / FIX-16 / FIX-18 / FIX-19 / FIX-20 / FIX-21**：SP-F2 后续批次

---

## SP-F1 交付摘要（参考）

| Commit | 内容 | 状态 |
|---|---|---|
| `000b432` | FIX-14: drop retired channel constants & dead listeners | ✅ |
| `f93f82c` | FIX-14 fixup: align stream_emitter banner & PermissionRequestPayload JSDoc | ✅ |
| `a07c2c2` | docs: SP-F1 plan + verification command fix | ✅ |
| `ee0cca5` | FIX-10: collapse duplicate listenToStream | ✅ |
| `9c02d8b` | FIX-11: introduce src/api/models.ts + migrate invoke sites | ✅ |
| `f1c26ed` | FIX-11 fixup: ActiveModel.auth_variant + style fixes | ✅ |
| `b6bf7d1` | FIX-17: stop re-exporting appendMessage/updateMessage | ✅ |

**总计**：7 commits，11 files changed，+491 / -153 lines。npm test 148/148 ✅。Final integrated review：APPROVED FOR MERGE。
