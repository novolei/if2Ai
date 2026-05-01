# SP-F1 — 机械清理批处理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 清理 ARCHITECTURE.md 审计报告（2026-05-02）中 P2 级"已退役但仍占据 API/常量表面"的死代码与冗余实现，降低后续 SP-A/B/C/D/E 的噪声与误用风险。

**Architecture:** 4 个互相独立的小 PR，每个 PR 一件事、一份 commit、独立可回滚。**不**改任何运行时行为，只删腐烂代码、合并双实现、薄迁移 invoke 调用。所有改动可被 `cargo check` + `npm run typecheck` + 现有测试套全量验证。

**Tech Stack:** TypeScript / React / Rust / Tauri 2 / Vitest / cargo test

**Branch strategy:** 在 `vnext` 上直接连续 4 个 commit 各自打 PR；不开 worktree（无设计判断、改动正交）。

**Source of truth for findings:**
- 上游审计报告已生成于本会话顶部（FIX-10 / FIX-14 / FIX-16 / FIX-17）
- 对应代码位置已逐项 grep 确认（见每个 Task 的 Files 段）

---

## Task 1: 删除已退役的 Tauri 频道常量与 dead listener (FIX-14)

**背景：** PR D-1（2026-05-02）已让 `runtime_event` 单频道在生产路径全面落地；`emit_payload`、`MemoryAuditEmitter`、`permission_service` 均不再发 `agent-token` / `permission-request` / `memory_event`。但 TS 端仍保留 3 个 `@deprecated` 常量与 2 个永远 no-op 的 listener；Rust 端仍保留 `AGENT_TOKEN_EVENT` 常量。这些是"语义真空"——下次新人调用 `listenToPermissionRequests` 会以为生效但永远收不到事件。

**Files:**
- Modify: `src/transport/contracts.ts:628-640` (删 3 个 `@deprecated` 常量与上方 6 行注释块)
- Modify: `src/lib/tauri.ts:17-22, 87-91, 105-115, 302-314` (删 import / re-export / 2 个 listener)
- Modify: `src-tauri/src/modules/runtime/stream_emitter.rs:41-43, 63-75` (删 `AGENT_TOKEN_EVENT` 常量与上方 doc 段)
- Verify (no edit): `src/App.tsx:1388`、`src/components/chat/TelemetryDrawer.tsx:483` 仅在注释里提到 listener 名字，不会断编译

- [ ] **Step 1.1: 全仓 grep 兜底确认无消费方**

```bash
rg -n "listenToPermissionRequests|listenMemoryEvent|AGENT_TOKEN_EVENT|PERMISSION_REQUEST_EVENT|\\bMEMORY_EVENT\\b" \
  --glob '!src/lib/tauri.ts' --glob '!src/transport/contracts.ts' \
  --glob '!src-tauri/src/modules/runtime/stream_emitter.rs'
```
Expected: 仅命中 `src/App.tsx:1388`（注释）、`src/components/chat/TelemetryDrawer.tsx:483`（注释）、可能命中测试文件中的字符串。**没有任何 import / 函数调用**。

- [ ] **Step 1.2: 删 TS 常量**

In `src/transport/contracts.ts`，删除 L628–L640 整段（注释块 + 3 个 `export const`）：

```ts
/** Canonical Tauri event names used by the agent loop. ... @deprecated ... */
export const AGENT_TOKEN_EVENT = 'agent-token'
export const PERMISSION_REQUEST_EVENT = 'permission-request'
export const MEMORY_EVENT = 'memory_event'
```

`MEMORY_AFTER_TURN_EVENT` 和 `RUNTIME_EVENT_CHANNEL`（紧邻在下方）**保留**——它们仍是活路径。

- [ ] **Step 1.3: 删 `src/lib/tauri.ts` 的 import / re-export / 两个 listener**

(a) L17–L22 import 块：删除 `MEMORY_EVENT,` 与 `PERMISSION_REQUEST_EVENT,` 两行，保留 `MEMORY_AFTER_TURN_EVENT` 和 `RUNTIME_EVENT_CHANNEL`。
(b) L87–L91 re-export 块：删除 `MEMORY_EVENT,` 与 `PERMISSION_REQUEST_EVENT,` 两行，保留 `RUNTIME_EVENT_CHANNEL`。
(c) L105–L115 删除整个 `listenMemoryEvent` 函数及其上方 doc 注释。
(d) L302–L314 删除整个 `listenToPermissionRequests` 函数及其上方 doc 注释。
(e) 检查 `MemoryEventPayload` / `PermissionRequestPayload` 类型 import 是否仍被本文件其它地方使用；若不再使用则一并从 type-import 块删除（grep `MemoryEventPayload\|PermissionRequestPayload` in tauri.ts 验证）。

- [ ] **Step 1.4: 删 Rust `AGENT_TOKEN_EVENT` 常量**

In `src-tauri/src/modules/runtime/stream_emitter.rs`：
(a) L63–L75 删除整个 `pub const AGENT_TOKEN_EVENT` 及其 doc 注释 + `#[deprecated]` 属性。
(b) L41–L43 doc 段第 3 条 hard rule（"The canonical event name [`AGENT_TOKEN_EVENT`] MUST be the only string in this crate that emits to that channel..."）整条删除，因为常量已不存在。

- [ ] **Step 1.5: typecheck + test (项目实际可用命令)**

`package.json` 没有 `typecheck` / `lint` 脚本。可用的检查是：

```bash
npx tsc --noEmit 2>&1 | rg -E "src/(lib/tauri|transport/contracts)\.ts|src-tauri/.+stream_emitter" || echo "OK: no errors in edited files"
npm test
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Expected: 第一行打印 `OK: no errors in edited files`（项目存在大量 pre-existing tsc 噪声，**只关心本 PR 编辑过的文件**是否新增报错）；后三个命令全部通过。

`cargo test --lib` 个别测试在并发负载下偶发 flake（如 `provider_api_key_probe_degraded_when_no_env_vars`）；若失败请重跑一次确认是 flake，不是本变更导致。

- [ ] **Step 1.6: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore(transport): drop retired Tauri channel constants & dead listeners (FIX-14)

PR D-1 (2026-05-02) collapsed agent-token / permission-request /
memory_event into the canonical runtime_event envelope. The TS-side
constants and the two no-op listeners (`listenMemoryEvent`,
`listenToPermissionRequests`) plus the Rust `AGENT_TOKEN_EVENT`
const have been kept "for one release cycle" since then. They are
now removed: no production path emits or subscribes to those names.

Refs: ARCHITECTURE.md §6, audit report 2026-05-02 (SP-F1 / FIX-14).
EOF
)"
```

---

## Task 2: 合并 `listenToStream` 双实现 (FIX-10)

**背景：** PR D-1 在 `src/api/streaming.ts:72-87` 实现了基于 `runtime_event` envelope 的 `listenToStream`。`src/lib/tauri.ts:285-300` 同时存在一份**逻辑等价**的副本（也已 D-1 化），是迁移半途遗留。`src/api/conversations.ts:37` 已经从 `./streaming.ts` import；`src/api/streaming.test.ts` 已覆盖 api 版本。`src/lib/tauri.ts` 版本**没有任何 import 方**（前一步 grep 已确认范围内只有该自身定义与 api 版本与 conversations.ts 的 api import）。

**Files:**
- Modify: `src/lib/tauri.ts:265-300` (删除 `listenToStream` 函数 + 其上方 JSDoc)

- [ ] **Step 2.1: grep 兜底确认 lib 版本无消费方**

```bash
rg -n "from ['\"]@/lib/tauri['\"]" -A 5 src/ | rg "listenToStream"
```
Expected: 无命中。`listenToStream` 的所有 import 都来自 `@/api/streaming` 或 `./streaming.ts`。

- [ ] **Step 2.2: 删 `src/lib/tauri.ts::listenToStream`**

删除 L265 附近 `/**` 起到 L300 结束 `}` 的整段（含 JSDoc）。同时检查文件顶部 import 块是否还在 import `RuntimeEventEnvelope`、`StreamTokenPayload`：若仅本函数使用则一并删除；若其它地方还用则保留。

- [ ] **Step 2.3: typecheck + 跑流式相关测试**

```bash
npx tsc --noEmit 2>&1 | rg -E "src/(lib/tauri|api/streaming|api/conversations)\.ts" || echo "OK: no errors in edited/related files"
npm test -- --test-name-pattern="listenToStream|conversations"
```

或全量 `npm test` 后人工确认 streaming/conversations 测试段通过。Expected: 全绿；编辑文件无新增 tsc 报错。

- [ ] **Step 2.4: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore(transport): collapse duplicate listenToStream — keep src/api/streaming.ts only (FIX-10)

PR D-1 left two D-1-compliant copies of listenToStream: one in
src/api/streaming.ts (the canonical facade, covered by
src/api/streaming.test.ts) and a second in src/lib/tauri.ts that
was the original migration site. No call site imports the lib copy
since src/api/conversations.ts already routes through ./streaming.ts.
Drop the duplicate so the facade has a single owner.

Refs: audit report 2026-05-02 (SP-F1 / FIX-10).
EOF
)"
```

---

## Task 3: 新增 `src/api/models.ts` + 迁移 3 处 `invoke('model_*')` (FIX-11)

**背景：** ARCHITECTURE.md §3.2 守则要求"新 UI 只走 `src/api/*`"，但当前 `App.tsx`、`chat-ui.tsx`、`HomeScreen.tsx` 仍直接 `invoke('model_get_active')` / `invoke('model_set_active', ...)`。同时 `App.tsx:2104-2112` 仍直接 `invoke('stop_agent_stream', ...)`，而 `src/api/streaming.ts:55-57` 已暴露 `stopAgentStream(streamId)`。

**Files:**
- Create: `src/api/models.ts`
- Modify: `src/api/index.ts` (re-export new module)
- Modify: `src/App.tsx:213-228` (use `getActiveModel`)
- Modify: `src/App.tsx:2104-2122` (use `stopAgentStream` from `@/api/streaming`)
- Modify: `src/components/ui/chat-ui.tsx:2459` (use `setActiveModel`)
- Modify: `src/modules/chat/components/HomeScreen.tsx:281` (use `setActiveModel`)

- [ ] **Step 3.1: Create `src/api/models.ts`**

Write file contents:

```ts
// MIG-012 / FIX-11 — model selection facade.
//
// Wraps the `model_get_active` / `model_set_active` Tauri commands so
// UI never imports `invoke` directly. The shape mirrors the backend
// `ActiveModel` snake_case wire.

import { getApiClient } from './client.ts'

export interface ActiveModel {
  provider_id: string
  model_id: string
}

/** Read the user's currently selected provider/model pair, or
 * `null` when nothing has been chosen yet (fresh install). */
export async function getActiveModel(): Promise<ActiveModel | null> {
  return getApiClient().call<ActiveModel | null>('model_get_active', {})
}

/** Persist a new provider/model selection. Backend emits
 * `if2ai://models-changed` on success — listeners in App.tsx /
 * chat-ui pick this up to refresh dependent UI. */
export async function setActiveModel(
  providerId: string,
  modelId: string,
): Promise<void> {
  return getApiClient().call<void>('model_set_active', {
    providerId,
    modelId,
  })
}
```

- [ ] **Step 3.2: Re-export from `src/api/index.ts`**

Read existing file first to follow current export style. Add a new line at the bottom:

```ts
export * from './models.ts'
```

(If the existing style uses named re-exports, match that style instead — read the file before editing.)

- [ ] **Step 3.3: Update `src/App.tsx` — `model_get_active`**

Locate the `refreshActiveModel` function around L214–L228. Replace the `invoke<{...}>('model_get_active')` call with `getActiveModel()`. Add `import { getActiveModel } from '@/api/models'` at the appropriate import block.

```ts
const activeModel = await getActiveModel()
if (activeModel) {
  setSelectedModel(`${activeModel.provider_id}/${activeModel.model_id}`)
}
```

- [ ] **Step 3.4: Update `src/App.tsx` — `stop_agent_stream`**

Locate `stopAgentStream` around L2104–L2122. Replace `invoke<string>('stop_agent_stream', { streamId: streamAbortHandle })` with `stopAgentStream(streamAbortHandle)` from `@/api/streaming`.

⚠️ **命名冲突注意**：本地函数也叫 `stopAgentStream`。**必须** rename 本地函数为 `stopAgentStreamForSession` 或 import 时用别名 `import { stopAgentStream as stopAgentStreamCommand } from '@/api/streaming'`。任选一种，但全文件保持一致。建议后者（最小化 diff）。

- [ ] **Step 3.5: Update `src/components/ui/chat-ui.tsx:2459`**

Replace `void invoke('model_set_active', { providerId: ..., modelId: ... })` with `void setActiveModel(providerId, modelId)`. Add `import { setActiveModel } from '@/api/models'`. 检查是否还有其它 `invoke(` 调用，若该文件其它地方仍需要 `invoke` 则保留 import；否则一并删除。

- [ ] **Step 3.6: Update `src/modules/chat/components/HomeScreen.tsx:281`**

同上。Replace 单行 `invoke('model_set_active', ...)` 用 `setActiveModel(parts[0], parts[1])`。Add import。检查并清理 `invoke` import 同上。

- [ ] **Step 3.7: typecheck + test**

```bash
npx tsc --noEmit 2>&1 | rg -E "src/(api/models|api/index|App|components/ui/chat-ui|modules/chat/components/HomeScreen|api/streaming)\.tsx?" || echo "OK: no errors in edited files"
npm test
```
Expected: 编辑文件无新增 tsc 报错；npm test 全绿。

- [ ] **Step 3.8: 启动 app 手动 smoke test（可选但强烈建议）**

如果 `npm run tauri dev` 可用，启动后：
1. 切换 model — 验证选择持久化、`models-changed` 监听器仍触发
2. 在流式 turn 中按 stop 按钮 — 验证 stream 真的停止

如果 smoke 不可用，跳过此步并在 commit message 注明 "tests + typecheck only; manual smoke deferred"。

- [ ] **Step 3.9: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
refactor(api): introduce src/api/models.ts and migrate 3 raw invoke sites (FIX-11)

ARCHITECTURE.md §3.2 forbids new UI from importing @/lib/tauri or
calling `invoke` directly. App.tsx still had two raw invoke sites
(`model_get_active`, `stop_agent_stream`) and chat-ui.tsx /
HomeScreen.tsx each had one (`model_set_active`).

This change:
  - Adds src/api/models.ts with `getActiveModel()` / `setActiveModel()`.
  - Re-exports from src/api/index.ts.
  - Migrates App.tsx::refreshActiveModel and App.tsx::stopAgentStream
    (renamed import to avoid local-function shadowing).
  - Migrates chat-ui.tsx and HomeScreen.tsx model_set_active sites.

No behavior change — pure facade move.

Refs: audit report 2026-05-02 (SP-F1 / FIX-11), ARCHITECTURE.md §3.2.
EOF
)"
```

---

## Task 4: 收回 `chat-store` 的 `appendMessage` / `updateMessage` 误用陷阱 (FIX-17)

**背景：** `conversation-slice.ts` 已给这两个函数打 `@deprecated`（L23–L24, L105, L128）；它们在生产路径**没有任何调用方**（grep 仅命中定义、test 与 `chat-store` 的 re-export）。但 `src/stores/chat-store.ts` L30/43/56/69 仍 re-export，意味着新代码可以 `import { appendMessage } from '@/stores/chat-store'` 并写入 transcript——这是个"诱饵 API"，与 SP-A 即将做的"transcript 单事实源"目标直接冲突。

**Files:**
- Modify: `src/stores/chat-store.ts:29-71` (删 2 个 import + 2 个 re-export)
- Modify: `src/stores/session-store.test.ts:11, 20, 117-130` (改从 `conversation-slice` 直接 import)

- [ ] **Step 4.1: grep 确认无其它生产消费方**

```bash
rg -n "from ['\"]@/stores/chat-store['\"]" src/ | rg "appendMessage|updateMessage"
```
Expected: 无命中（即没有除测试外的代码 import 这两个名字 from chat-store）。若有命中：**停下来报告**——这意味着 SP-A 之前还有真实的旁路写 transcript 路径需要处理。

- [ ] **Step 4.2: 修改 `src/stores/chat-store.ts`**

(a) L29–L46 import 块：删除 `appendMessage,` 与 `updateMessage,` 两行。
(b) L55–L71 re-export 块：删除 `appendMessage,` 与 `updateMessage,` 两行。
(c) 在文件顶部 doc 注释（L14–L25 的 "What this file deliberately does NOT do" 段）追加一条：

```ts
// - It does NOT re-export `appendMessage` / `updateMessage`.
//   Transcript mutations are owned by the runtime-projection
//   pipeline (events → reducer → snapshot). The slice still
//   exposes them under @deprecated for emergency back-compat,
//   but no production code path should reach them.
```

- [ ] **Step 4.3: 修改 `src/stores/session-store.test.ts`**

L11、L20 处的 import：把 `appendMessage` 与 `updateMessage` 从 `'@/stores/chat-store'`（实际路径以文件中现有写法为准；可能是 `'./chat-store'`） 改为从 `'./conversation-slice'` 直接 import。其余测试 body 不变。

如果 import 写法是 `from './chat-store'` 集合块：把 `appendMessage` / `updateMessage` 拆出来，单独 `import { appendMessage, updateMessage } from './conversation-slice'`。

- [ ] **Step 4.4: typecheck + test**

```bash
npx tsc --noEmit 2>&1 | rg -E "src/stores/(chat-store|conversation-slice|session-store)" || echo "OK: no errors in edited files"
npm test
```
Expected: 编辑文件无新增 tsc 报错；session-store.test.ts 仍通过。

- [ ] **Step 4.5: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore(stores): stop re-exporting appendMessage/updateMessage from chat-store (FIX-17)

The conversation-slice already marks both functions @deprecated and no
production code path calls them — only session-store.test.ts uses them
to assert the slice mutation contract. Re-exporting from chat-store
created a "decoy API": new code could `import { appendMessage } from
'@/stores/chat-store'` and silently introduce a second transcript
truth source, defeating the runtime-projection pipeline.

Tests now import the deprecated mutations directly from the slice;
the slice still owns them for emergency back-compat, but they no
longer surface on the canonical chat-store facade.

This is a pre-requisite cleanup for SP-A (chat transcript single
source of truth).

Refs: audit report 2026-05-02 (SP-F1 / FIX-17).
EOF
)"
```

---

## Final verification (after all 4 tasks)

- [ ] **Final.1: 全套测试**

```bash
npx tsc --noEmit 2>&1 | tee /tmp/sp-f1-tsc.log | tail -5
npm test
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: `npm test` / `cargo check` / `cargo test` 全绿。`tsc --noEmit` 会有 pre-existing 噪声，但**与本批 PR 涉及的文件无关**——核对 `/tmp/sp-f1-tsc.log` 中的报错，确认没有任何条目指向 SP-F1 编辑过的 10 个文件。

- [ ] **Final.2: `git log --oneline` 检查 4 个 commit 顺序与 message 风格**

```bash
git log --oneline -6
```
Expected: 4 个新 commit，每个 message 都 include `Refs: audit report 2026-05-02`，subject 都以 `chore(...)` / `refactor(...)` 起始，与仓库现有 PR D-1 / D-2 风格一致。

- [ ] **Final.3: `git diff vnext..HEAD --stat` 确认只动了预期文件**

```bash
git diff origin/vnext..HEAD --stat
```
Expected: 只有以下文件出现：
- `src/transport/contracts.ts`
- `src/lib/tauri.ts`
- `src-tauri/src/modules/runtime/stream_emitter.rs`
- `src/api/models.ts` (new)
- `src/api/index.ts`
- `src/App.tsx`
- `src/components/ui/chat-ui.tsx`
- `src/modules/chat/components/HomeScreen.tsx`
- `src/stores/chat-store.ts`
- `src/stores/session-store.test.ts`

任何超出此清单的文件出现 → 停下来报告（可能是 IDE 自动 format 误伤）。

---

## Out of scope (deferred to other subprojects)

明确**不在**本 plan 中：
- FIX-1 / FIX-2 / FIX-3 / FIX-4（chat transcript 真源）→ SP-A
- FIX-5 / FIX-13（supervisor 接通）→ SP-B
- FIX-7 / FIX-9（流式日志统一 / 反向依赖）→ SP-C
- FIX-8（god-file 拆分）→ SP-D
- FIX-6（harness 派生）→ SP-E
- FIX-12 / FIX-18（命令瘦身 / UI api 迁移）→ 后续机会主义小 PR
- FIX-15 / FIX-16 / FIX-19 / FIX-20 / FIX-21 → SP-F2 后续批次（本批先吃掉风险最低的 4 项）

任何在本批次执行中**新发现**的问题，写到 `docs/superpowers/plans/2026-05-02-sp-f1-followups.md`，**不**夹带进当前 4 个 PR。

---

## Self-review (writing-plans skill 要求)

✅ Spec coverage：FIX-14、FIX-10、FIX-11、FIX-17 各对应 1 个 Task；FIX-16 拆出文档相关部分到后续批次（明确写在 Out of scope）。
✅ Placeholder scan：无 TBD / TODO / "implement later"；每个 step 都给了具体代码 / 命令 / 文件:行号。
✅ Type consistency：Task 3 的 `ActiveModel` 接口与 Rust 端 snake_case 字段对齐（`provider_id` / `model_id`）；`getActiveModel` / `setActiveModel` 函数名前后一致。
✅ 命名冲突：Task 3.4 显式提示 `stopAgentStream` 命名 shadow 问题与解决方案。

---

**End of SP-F1 plan.**
