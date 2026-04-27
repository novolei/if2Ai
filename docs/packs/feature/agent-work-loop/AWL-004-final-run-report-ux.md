# AWL-004: FinalRunReport UX

## Status
- State: active

## Goal
Make every run ending visible and recoverable in the chat UI. Success, failure,
approval blocked, provider error, and budget exhaustion should all render a coherent
summary from runtime projection.

## Spec (verifiable)
- Success report shows completed work, loop kind, skills, and evidence summary -> test `src/runtime-projection/final-run-report.test.ts`.
- Failure report shows failed point, retry guidance, and next user action -> test `src/runtime-projection/final-run-report.test.ts`.
- Approval-blocked report shows operation, risk reason, parameters, and approval state -> test `src/components/chat/RunInspectorPanel.test.tsx`.
- Mobile layout keeps report controls and labels within their containers -> e2e step `e2e/final-run-report.spec.ts`.

## Files (scope)
- `src/components/chat/RunInspectorPanel.tsx`
- `src/components/ui/chat-ui.tsx`
- `src/modules/chat/components/ChatWorkspace.tsx`
- `src/modules/chat/types.ts`
- `src/runtime-projection/types.ts`
- `src/runtime-projection/runtime-event-translator.ts`
- `src/runtime-projection/runtime-event-reducer.ts`
- `src/transport/contracts.ts`

## Reads
- `docs/design-docs/agent-work-loop/AWL-004-final-run-report-ux.md`
- `src/modules/chat/components/*`
- `src/components/ui/*`

## Contract
- Runtime projection remains the only frontend truth.
- Do not create a second report store or localStorage state.
- Do not rename existing event types.
- Keep UI dense and work-focused; no landing-page treatment.

## Out of Scope
- New backend report fields unless needed for typed compatibility.
- Full approval-sheet redesign.
- MCP Workbench UI.

## Verify
- `npm run build:web`
- `npm run test -- final-run-report`
- `cargo check --manifest-path src-tauri/Cargo.toml`

## Done
- Verify commands pass.
- `REGISTRY.md` moves this pack to done with date.
