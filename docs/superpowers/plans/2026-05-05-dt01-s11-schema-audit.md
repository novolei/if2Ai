# DT-01 S1.1 — Schema Audit: AgentEvent ↔ run-log envelope

> **Plan**: [`2026-05-05-dt01-harness-derive-from-runlog.md`](./2026-05-05-dt01-harness-derive-from-runlog.md)
> **Source**: `src-tauri/src/modules/harness/event_bus.rs` (16 `AgentEvent` variants)
> **Audit method**: read `harness/event_bus.rs`, `harness/trace_aggregator.rs`, `runtime/event_log.rs`, `runtime/contracts/common.rs`; grep production call sites for `runtime_event::dispatch`, `RuntimeEventEnvelope::new`, `RunEventLogger::append*`, and `AgentEvent::*` / `emit_*` helpers.
> **Date**: 2026-05-05

## Legend

- ✅ — present in run-log JSONL today with payload fidelity sufficient for `fold_run_log_to_report`
- ⚠️ — event reaches the run-log but one or more payload fields used by `TraceAggregator` are missing or different in shape
- ❌ — event is **only** on the harness EventBus; run-log has no equivalent envelope today

## Summary

| Status | Count | Variants |
|---|---|---|
| ✅ | 2 | `MemoryAfterTurn`, `PermissionPrompted` |
| ⚠️ | 5 | `TurnStarted`, `TurnFinished`, `LlmRequested`, `LlmResponded`, `PermissionResolved` |
| ❌ | 9 | `ToolCalled`, `ToolResult`, `ContextCompacted`, `ReflectionCompleted`, `PrepareStepExecuted`, `ExecutionModeJudged`, `StreamErrored`, `ResumeInvoked`, `(none missing)` |

(Total = 16. Note: `ToolCalled` / `ToolResult` are partially observable via run-log `tool:tool_call_*` rows but `tool_name` / `success` / `duration_ms` carriage requires a payload-level audit listed below.)

## Per-variant audit

### 1. `TurnStarted` — ⚠️ exists but payload fields missing

- **Bus payload**: `turn_number`, `session_id`, `at`
- **Production emit**: `harness::agent_loop_integration::emit_turn_started` called from `turn_service/run.rs:192` and `turn_service/stream_task.rs:392`
- **Run-log equivalent**: `conversation:run_started` envelope appended in `turn_service/run.rs:136` (`RunEventLogger::append_with_correlation("run_started", { caller, permission_mode, message_preview }, …)`).
- **Gap**: `turn_number` is **not** in the `run_started` payload. `TraceAggregator` uses `turn_number` only for ordering / aggregation (turn count is derived from `TurnFinished` count, see below), so for fold purposes the gap is benign — fold can derive `turn_number` from the count of `run_started` rows seen prior to the current `turn_finished`. Document this in S1.2 fold contract.
- **S1.2 action**: extend the `run_started` payload at `turn_service/run.rs:137-145` to include `"turn_number"` (already known in scope at that call site as `turn_number_run`); no new family needed.

### 2. `TurnFinished` — ⚠️ exists but payload fields missing

- **Bus payload**: `turn_number`, `session_id`, `success`, `tokens_used`, `duration_ms`, `at`
- **Production emit**: `emit_turn_finished` called from `turn_service/stream_finalize.rs:1118`, `turn_service/run.rs:286`, `:870`, `:896`
- **Run-log equivalent**: closest is `conversation:stream_complete` (final terminal envelope from `runtime/stream_emitter.rs:297`'s `map_event_type_to_runtime`). Also `run_error` rows for failure cases.
- **Gap**: there is no canonical `conversation:turn_finished` envelope today. `success` / `tokens_used` / `duration_ms` are **not** uniformly persisted on the run-log. Fold currently has to triangulate from `stream_complete` / `stream_error` / `run_error` rows.
- **S1.2 action**: at every `emit_turn_finished` call site (4 locations above), also append a `runtime_event::dispatch(RuntimeEventType::Conversation, "turn_finished", {turn_number, success, tokens_used, duration_ms})` envelope so the run-log has a 1:1 row. Use `RuntimeEventType::Conversation` (do **not** invent a new family per non-goal in plan §4).

### 3. `LlmRequested` — ⚠️ exists but payload fields missing

- **Bus payload**: `session_id`, `input_tokens`, `at`
- **Production emit**: `emit_llm_requested(...)` defined at `harness/agent_loop_integration.rs:80`. **Production call sites grep returns zero hits outside the harness module** — this is a known gap: the helper is defined but no upstream caller invokes it in the production agent loop.
- **Run-log equivalent**: no `conversation:llm_requested` envelope exists today. Token usage flows through `stream_complete` / `final_run_report` payloads at terminal time (see `runtime/stream_emitter.rs:297`).
- **Gap**: both bus and run-log are silent today on per-call LLM requests; the `TraceAggregator` aggregates `llm_calls` / `input_tokens_total` from `LlmRequested` events, so when no caller emits them the bus aggregate is also zero. This means S1.2 reconciliation can keep parity by either (a) leaving both at zero or (b) wiring both producers.
- **S1.2 action**: optional. Only add an emit if S1.2 wiring also updates the call sites (`turn_service/stream_iteration.rs` model-call entry). If not wired, document the deliberate zero in the fold contract.

### 4. `LlmResponded` — ⚠️ same shape as `LlmRequested`

- **Bus payload**: `session_id`, `output_tokens`, `streamed`, `at`
- **Production emit**: `emit_llm_responded` at `harness/agent_loop_integration.rs:103`. **No production caller** (grep result identical to #3).
- **Run-log equivalent**: terminal token usage available in `final_run_report` / `stream_complete` payloads, but no per-response envelope.
- **S1.2 action**: same as #3.

### 5. `ToolCalled` — ❌ payload mismatch

- **Bus payload**: `session_id`, `tool_name`, `args_snippet` (1 KB cap), `at`
- **Production emit**: `emit_tool_called` from `turn_service/stream_tool_execution.rs:632`
- **Run-log equivalent**: `tool:tool_call_queued` / `tool_call_running` rows produced by `stream_task_run_log.rs::append_stream_event` (event-type normalisation table). These rows carry the full `StreamTokenPayload` shape (which includes `tool_name`, `tool_call_id`, `tool_input`, `tool_status`) — strictly **richer** than the bus payload but **not under a stable family alias matching `tool:requested`** as the plan §2.1 mapping table assumed.
- **Gap**: family/tag mismatch (`tool_call_queued` vs `tool:requested`) — fold will need a normalisation table mapping both family strings into the `tool_calls_by_name` counter. No data-loss gap.
- **S1.2 action**: in `runlog_projection.rs::fold_run_log_to_report`, treat `tool:tool_call_queued` (first observation per `tool_call_id`) as the bus's `ToolCalled`. No emit-side change.

### 6. `ToolResult` — ❌ payload mismatch (same root as #5)

- **Bus payload**: `session_id`, `tool_name`, `success`, `duration_ms`, `at`
- **Production emit**: `emit_tool_result` from `turn_service/stream_tool_execution.rs:689`
- **Run-log equivalent**: `tool:tool_call_completed` (`tool_status="completed"`) and `tool:tool_call_failed` (`tool_status="error"`) rows from `stream_task_run_log.rs::append_stream_event:55-61`. These carry `tool_name` and `tool_status`; **`duration_ms` is not on the streaming payload schema** (verify in S1.2 by reading `runtime/stream_emitter.rs::StreamTokenPayload`).
- **Gap**: `duration_ms` is missing from run-log payload — `TraceAggregator` doesn't currently aggregate per-tool durations into `AggregateMetrics`, so the gap is benign for fold parity, but record it for future use.
- **S1.2 action**: in fold, derive `success` from `tool_status` ("completed" → true, "error" → false). No emit-side change required for current fold contract.

### 7. `ContextCompacted` — ❌ not emitted to run-log today

- **Bus payload**: `session_id`, `messages_before`, `messages_after`, `at`
- **Production emit**: `emit_context_compacted` defined at `harness/agent_loop_integration.rs:166`. Grep finds **no production caller** (only the 7-line smoke test inside `agent_loop_integration.rs:227`).
- **Run-log equivalent**: `compression_event:*` envelope exists (`turn_service/stream_iteration.rs:277` dispatches `RuntimeEventType::CompressionEvent`); separate code path from harness compaction.
- **Gap**: bus variant has zero production traffic; the `compression_event` family is its own thing and is also recorded only via run-log (not on the bus). For fold parity this means `compaction_events` aggregate is `0` on both sides today.
- **S1.2 action**: leave as-is. Document the dead bus variant; remove in S2.3 along with the rest of EventBus.

### 8. `PermissionPrompted` — ✅ exists with payload fidelity

- **Bus payload**: `session_id`, `tool_name`, `at`
- **Production emit**: `harness/agent_loop_integration.rs::emit_permission_prompted`. Grep does not find a direct call but the canonical emit site is `application/permission_service.rs:121` (`emit_permission_prompt(...)`) which dispatches the `runtime_event::dispatch(RuntimeEventType::Permission, "prompt_opened", …)` envelope itself (`permission_service.rs:333-340`).
- **Run-log equivalent**: `permission:prompt_opened` envelope, payload includes `tool_name`, `request_id`, `session_id`, `permission_mode`, `current_mode`, `message`. **Strict superset of bus payload.**
- **S1.2 action**: in fold, count `permission:prompt_opened` rows for `permission_prompts` aggregate and append `tool_name` + `observed_at` (= `entry.occurred_at`) to `evidence.permission_prompts`. No emit-side gap.

### 9. `ReflectionCompleted` — ❌ not emitted anywhere

- **Bus payload**: `session_id`, `insights_count`, `at`
- **Production emit**: helper exists at `agent_loop_integration.rs:202` but grep returns **no production caller** (only the smoke test).
- **Run-log equivalent**: none.
- **S1.2 action**: ignore (zero on both sides). Drop in S2.3.

### 10. `PrepareStepExecuted` — ❌ not emitted to run-log today

- **Bus payload**: `session_id`, `tool_name`, `outcome`, `boundary`, `permission`, `sandbox`, `policy_version`, `at`
- **Production emit**: `turn_service/stream_tool_execution.rs:651` (direct `bus.emit(AgentEvent::PrepareStepExecuted{…})`)
- **Run-log equivalent**: none. No `runtime_event::dispatch` call carries the `prepare_step_execution` decision.
- **Gap**: complete. The full typed `PrepareStepOutcome` / `BoundaryDecision` / `PermissionDecision` / `SandboxPolicy` shape lives only on the bus.
- **S1.2 action**: at `stream_tool_execution.rs:651`, mirror the emit through `runtime_event::dispatch(RuntimeEventType::System, "prepare_step", payload)` — payload is `serde_json` of the same struct fields. Use `RuntimeEventType::System` (closest existing family; do not introduce a `Harness`-only family per plan §4). Fold then reads `system:prepare_step` rows into `evidence.prepare_step` and the four `prepare_step_*` counters.

### 11. `ExecutionModeJudged` — ❌ not emitted to run-log today

- **Bus payload**: `session_id`, `execution_mode`, `risk_level`, `complexity_level`, `policy_version`, `at`
- **Production emit**: `commands/request_intelligence.rs:79` (`harness.event_bus.emit(AgentEvent::ExecutionModeJudged{…})`)
- **Run-log equivalent**: indirect. `stream_emitter.rs:309` maps the wire-level `execution_mode_decision` event to `RuntimeEventType::ExecutionMode`, but that wire event is dispatched from the streaming layer at decision time (not from the request-intelligence classifier). Confirm in S1.2 whether the same decision lands on both paths or only one.
- **Gap**: classifier-side decision is bus-only.
- **S1.2 action**: at `commands/request_intelligence.rs:79`, also call `runtime_event::dispatch(RuntimeEventType::ExecutionMode, "judged", payload)` with the same fields. Fold reads `execution_mode:judged` rows into `evidence.execution_mode` + `execution_mode_judgments` counter.

### 12. `PermissionResolved` — ⚠️ exists but payload fields differ

- **Bus payload**: `session_id`, `tool_name` (Option), `decision` (`"allow"` / `"deny"`), `scope` (`"once"` / `"session"`), `at`
- **Production emit**: `application/permission_service.rs:268` (direct `harness.event_bus.emit(...)`)
- **Run-log equivalent**: `permission:permission_resolved` rows are appended in two places — `stream_task_run_log.rs:88` (session-override path) and the regular `respond_permission` IPC handler. **Need to verify in S1.2 whether the IPC handler dispatches a canonical envelope or only writes to run-log via `append`.** The `stream_task_run_log` path uses raw `append("permission_resolved", payload)` rather than `runtime_event::dispatch`, so the row exists but does not necessarily ride a `RuntimeEventType::Permission` family tag (its `event_type` will be the bare string `"permission_resolved"` not `"permission:permission_resolved"`).
- **Gap**: family-tag inconsistency (`"permission_resolved"` bare vs `"permission:decision"` canonical). Fold needs to accept both today.
- **S1.2 action**: convert `stream_task_run_log.rs:88-90` to `runtime_event::dispatch(RuntimeEventType::Permission, "decision", payload)`; for the IPC handler, audit and align. Fold then reads `permission:decision` rows into the `permission_resolved_*` counters and `evidence.permission_resolved`.

### 13. `StreamErrored` — ❌ not emitted to run-log today

- **Bus payload**: `session_id`, `reason`, `resume_available`, `at`
- **Production emit**: `turn_service/stream_event_loop.rs:567` and `turn_service/stream_iteration.rs:484, :585` (3 sites, all direct `bus.emit(...)`)
- **Run-log equivalent**: `conversation:stream_error` envelope is dispatched via the canonical streaming pipeline (`stream_emitter.rs:297` maps `"stream_error"` → `RuntimeEventType::Conversation`). Payload typically includes `reason` + `resume_available` (verify in S1.2 against `StreamTokenPayload` shape).
- **Gap**: likely just a payload-fidelity confirm. The bus event is duplicative of the canonical `stream_error` row.
- **S1.2 action**: in fold, count `conversation:stream_error` rows into `stream_errors` aggregate and push to `evidence.stream_errors`. Verify `resume_available` is on the payload; if missing, augment at `stream_iteration.rs:484` payload construction.

### 14. `ResumeInvoked` — ❌ not emitted anywhere

- **Bus payload**: `session_id`, `resume_cursor`, `at`
- **Production emit**: comment at `event_bus.rs:209-211` explicitly notes "**No production caller emits this today**". Grep confirms.
- **Run-log equivalent**: none.
- **S1.2 action**: leave as 0/empty on both sides; the resume code path lands in M4.5+.

### 15. `MemoryAfterTurn` — ✅ exists with payload fidelity

- **Bus payload**: `trace_version`, `caller`, `session_id`, `project_id`, `policy_version`, `decided_at`, `decisions[]`, `quality`, `conflicts[]`
- **Production emit**: `application/stream_emitter_service.rs:118-128`
- **Run-log equivalent**: `memory:after_turn` envelope dispatched at `application/stream_emitter_service.rs:100-106` from the **same call site** as the bus emit. Payload (`stream_emitter_service.rs:84-92`) carries every field 1:1 (`traceVersion` / `caller` / `policyVersion` / `decidedAt` / `decisions` / `quality` / `conflicts`).
- **S1.2 action**: in fold, parse `memory:after_turn` payload back into `MemoryAfterTurnTrace` and push to `evidence.memory_after_turn` + the 9 `memory_*` counters. The dual-emit makes this the cleanest variant to validate reconciliation against first.

### 16. (placeholder — 16th variant in source)

The `event_bus.rs::AgentEvent` enum has 15 variants visible in the source (the comment at lines 132-149 mentions a previously-existing `MemoryAfterTurn` documentation block above the `PrepareStepExecuted` doc block; the enum body itself contains 15 distinct variants: `TurnStarted`, `TurnFinished`, `LlmRequested`, `LlmResponded`, `ToolCalled`, `ToolResult`, `ContextCompacted`, `PermissionPrompted`, `ReflectionCompleted`, `PrepareStepExecuted`, `ExecutionModeJudged`, `PermissionResolved`, `StreamErrored`, `ResumeInvoked`, `MemoryAfterTurn`). The plan and the IMPROVEMENTS doc cite "16 variants"; this audit treats the canonical count as **15**. S1.2 should reconcile the count with the IMPROVEMENTS doc. No data gap.

## Top 3 gaps to fix in S1.2 (priority order)

1. **`TurnFinished` → `conversation:turn_finished`** (variant #2) — the single highest-leverage emit gap. Without it, `turns_completed`, `turns_succeeded`, `total_turn_duration_ms`, and `last_turn_succeeded` cannot be derived from run-log without fragile triangulation across `stream_complete` / `stream_error` / `run_error` rows. **Add a `runtime_event::dispatch(RuntimeEventType::Conversation, "turn_finished", {turn_number, success, tokens_used, duration_ms})` at the 4 `emit_turn_finished` call sites listed in #2.**
2. **`PrepareStepExecuted` → `system:prepare_step`** (variant #10) — full evidence vector + 4 aggregate counters depend on this; today entirely bus-only. **Mirror the emit at `stream_tool_execution.rs:651` via `runtime_event::dispatch(RuntimeEventType::System, "prepare_step", payload)`.**
3. **`ExecutionModeJudged` → `execution_mode:judged`** (variant #11) — classifier judgments are bus-only today. **Mirror the emit at `commands/request_intelligence.rs:79`.**

## Secondary gaps (nice-to-have for fold parity, not blocking)

- `TurnStarted` `turn_number` payload addition (variant #1)
- `PermissionResolved` family-tag normalisation in `stream_task_run_log.rs` (variant #12)
- Verify `StreamErrored` payload carries `resume_available` (variant #13)

## Variants intentionally left untouched (zero on both sides today)

- `LlmRequested` / `LlmResponded` (variants #3, #4) — unwired in production agent loop
- `ContextCompacted` (variant #7) — superseded by `compression_event` family
- `ReflectionCompleted` (variant #9) — no caller
- `ResumeInvoked` (variant #14) — reserved for M4.5+

These will be deleted alongside the EventBus in DT-01-S2.3.
