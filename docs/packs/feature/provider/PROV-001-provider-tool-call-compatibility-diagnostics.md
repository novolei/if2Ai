# PROV-001: Provider Tool Call Compatibility Diagnostics

## Status
- State: done

## Goal
Detect and explain provider/model responses that look like tool calls in text
but do not arrive as canonical tool-call events.

## Spec (verifiable)
- Fake textual `<function_calls>` / `<invoke name=...>` output is detected as a provider compatibility warning -> test `provider_textual_tool_call_markup_detected`.
- Tool-required runs with fake tool markup still end through AWL-010 evidence rules, not completed -> test `textual_tool_markup_without_tool_event_stays_failed`.
- Runtime event log records the provider id, model id, markup family, and recovery action without storing unsafe raw payloads -> test `provider_tool_call_compat_warning_is_sanitized`.
- Run Inspector / final report can show "provider produced textual tool markup" as a diagnostic warning.

## Files (scope)
- `docs/design-docs/provider/PROV-001-provider-tool-call-compatibility-diagnostics.md`
- `docs/packs/feature/provider/PROV-001-provider-tool-call-compatibility-diagnostics.md`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/application/turn_service/stream_event_loop.rs`
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`
- `src-tauri/src/modules/runtime/stream_outcome.rs`
- `src-tauri/src/modules/runtime/contracts/agent_loop.rs`
- `src/components/chat/RunInspectorPanel.tsx`
- `src/runtime-projection/final-run-report.test.ts`

## Reads
- `docs/packs/feature/agent-work-loop/AWL-010-continuation-context-tool-evidence-invariants.md`
- `docs/design-docs/agent-work-loop/AWL-010-continuation-context-tool-evidence-invariants.md`

## Contract
- Do not execute text that merely resembles a tool call.
- Do not add provider-specific branches outside provider/stream diagnostics seams.
- Do not change IPC command names or sqlite schema.

## Out of Scope
- Provider client rewrites.
- MCP transport work.
- Prompt/persona changes.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml provider_tool_call_compat -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml application::turn_service -- --nocapture`
- `npm run test -- final-run-report`
- `cargo check --manifest-path src-tauri/Cargo.toml`

## Done
- Diagnostics explain fake tool-call markup and AWL-010 completion invariants remain intact.
