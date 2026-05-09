# `CorrelationIds` Field Population Convention

> **DR-04** — canonical guide for which fields to populate at each emit site so consumers (frontend projection, harness aggregator, run log) can reconstruct full event chains.
>
> **Audience:** anyone adding a new `runtime_event::dispatch` call or `RuntimeEventEnvelope` construction.
> **Source of truth:** [`src-tauri/src/modules/runtime/contracts/common.rs`](../../src-tauri/src/modules/runtime/contracts/common.rs) struct `CorrelationIds`.

---

## TL;DR

Every emit site **MUST** populate at least the fields that are observable at that point. Defaulting to `CorrelationIds::default()` (everything `None`) is the anti-pattern this doc exists to prevent — it drops signal that consumers expect.

```rust
use crate::modules::runtime::contracts::common::CorrelationIds;

// ❌ BAD — drops every observable id
let _ = runtime_event::dispatch(handle, event_type, family, CorrelationIds::default(), &payload, None);

// ✅ GOOD — emit what you know
let correlation = CorrelationIds {
    session_id: Some(session_id.clone()),
    run_id: Some(run_id.clone()),
    turn_index: Some(turn_index),
    ..Default::default()
};
let _ = runtime_event::dispatch(handle, event_type, family, correlation, &payload, None);
```

---

## Field-by-field convention

### Always populate when known

| Field | Type | When to populate |
|-------|------|------------------|
| **`session_id`** | `Option<String>` | Every emit site that runs inside a chat session. The supervisor / app-shell creates this on session open. The exception is genuinely session-less events (boot phase change, daydream cycle from idle trigger, scheduler tick). |
| **`run_id`** | `Option<String>` | Every emit inside a single `run_turn` / `run_stream_task` invocation. The IPC adapter creates a fresh `Uuid` at the top of `run_turn`; pass it into every helper that needs to emit. |
| **`stream_id`** | `Option<String>` | Streaming path only. Kept for backwards compatibility with the M0.x agent-token channel; new code should prefer `run_id` but keep emitting `stream_id` until M2 cut-over completes. |

### Populate when entering a sub-scope

| Field | Type | When to populate |
|-------|------|------------------|
| **`turn_index`** | `Option<u32>` | When emitting from the i-th iteration inside a multi-iteration run (preflight retry, tool loop iteration). 0 for the first iteration. Leave `None` if the event is run-scoped, not turn-scoped. |
| **`attempt_id`** | `Option<String>` | When emitting a tool-call event. Each tool invocation attempt gets a distinct `attempt_id`. The attempt ledger (T-013) tracks `attempt_no` separately for monotonic numbering. Don't reuse `attempt_id` across retries. |

### Populate when project context is observable

| Field | Type | When to populate |
|-------|------|------------------|
| **`project_id`** | `Option<String>` | When the session is bound to a project (most user sessions). The control plane resolves this via `SessionContextResolver` early in `prepare_step_execution`. Pass it through. |

### Agents Teams (Wave B+ — see `ARCHITECTURE.md §9.3`)

These fields exist in the contract from M0.3 even though no `Team` entity exists in code yet. **Today: leave as `None`.** When the team bounded context ships, a TeamSupervisor will populate them.

| Field | Type | Future use |
|-------|------|------------|
| **`team_id`** | `Option<String>` | The team this run belongs to. Populated by TeamSupervisor when a team-mode session starts. |
| **`member_id`** | `Option<String>` | Which team member (agent persona) is producing this event. Populated when the TeamSupervisor delegates to a sub-agent. |
| **`role_id`** | `Option<String>` | Role within the team (planner / executor / reviewer / etc.). Same population point as `member_id`. |
| **`parent_run_id`** | `Option<String>` | When a sub-agent run is spawned from a parent run, this points back. Lets harness / projection reconstruct the delegation graph. |
| **`delegation_id`** | `Option<String>` | The specific delegation event that spawned this sub-run. Distinct from `parent_run_id` when one parent delegates multiple times. |

---

## Inheritance helpers

Most production emit sites inherit `CorrelationIds` from a parent context (the run logger, the supervisor snapshot, the stream emitter). Use the inheritance pattern instead of constructing fresh:

```rust
// At the top of run_stream_task_body, build once:
let log_correlation = CorrelationIds {
    session_id: Some(inputs.session_id.clone()),
    run_id: Some(inputs.run_event_logger.run_id().to_string()),
    stream_id: Some(inputs.stream_id_for_task.clone()),
    project_id: inputs.execution_context_for_task.project_id.clone(),
    ..Default::default()
};

// Then deeper in the loop, only override what changes:
let per_iteration = CorrelationIds {
    turn_index: Some(iteration as u32),
    ..log_correlation.clone()
};
```

This keeps the "what's known" contract honest: the iteration knows its turn_index, the per-tool emit additionally knows its `attempt_id`, and the run-level emit knows neither.

---

## Emit-site-by-emit-site checklist

Common emit sites and the minimum fields each MUST populate:

| Emit site | Required | Optional |
|-----------|----------|----------|
| `runtime_event::dispatch` from `commands::agent::run_agent_turn` | `session_id`, `run_id` | `project_id` if known |
| `runtime_event::dispatch` from `commands::agent::start_agent_stream` | `session_id`, `run_id`, `stream_id` | `project_id` |
| Per-iteration emit inside `run_stream_task_body` | session_id, run_id, stream_id, **`turn_index`** | project_id, attempt_id (for tool events) |
| Tool execution event (`tool_call_started` / `tool_call_completed` / `tool_call_failed`) | session_id, run_id, **`attempt_id`** | turn_index |
| Permission prompt (`permission:prompt_opened` / `permission:resolved`) | session_id, run_id | turn_index, attempt_id (for the tool that triggered the prompt) |
| Memory event (capture / recall / write_decision) | session_id (if user-initiated) | run_id, project_id |
| Harness event (TurnStarted / TurnFinished / ToolResult) | session_id, run_id, turn_index | attempt_id |
| Supervisor lifecycle event | session_id | run_id (for run-scoped transitions: start_run / completed / failed / cancelled) |
| Daydream cycle (`daydream_cycle:completed` / `daydream_cycle:failed`) | nothing — the cycle is global / opt-in | optional `session_id` if triggered manually from a session UI |
| Boot / system event (boot phase, daemon health) | nothing — pre-session | nothing |

---

## Anti-patterns

### `CorrelationIds::default()` at a non-system event site

If your event happens inside a session, your event MUST carry `session_id`. The `default()` shortcut is reserved for boot / system / scheduler events that have no session.

### Constructing `CorrelationIds` from scratch when a parent is in scope

If you can reach a `RunEventLogger`, `SupervisorSnapshot`, or `FinalizeStreamInputs`, **inherit from it** rather than constructing fresh. The parent already has the right `session_id` / `run_id` / etc. Hand-constructing risks divergence.

### Populating Teams fields with mock values "for forward compatibility"

Don't. The contract treats absence as "not applicable", not "unknown" (per the struct doc-comment line 156). Hand-populated `team_id: Some("none".into())` would mislead consumers into thinking the run was team-scoped.

### Using `stream_id` on the non-streaming path

`stream_id` is for streaming-only emits. The non-streaming `run_turn` path should use `run_id` exclusively. Mixing the two confuses consumers that filter by one or the other.

---

## Future (after Teams ships)

Once the `team` bounded context lands (Wave B+), these become required at team-scoped emit sites:

| Then-required field | At which emit sites |
|---------------------|---------------------|
| `team_id` | every emit inside a TeamSupervisor invocation |
| `member_id` | sub-agent emits (the executor / reviewer / etc.) |
| `role_id` | same as member_id; the role at the time of emission |
| `parent_run_id` | every sub-agent run spawned by a delegation |
| `delegation_id` | the specific event-log entry that recorded the delegation |

This doc will be updated when that lands. Until then, leave them `None`.
