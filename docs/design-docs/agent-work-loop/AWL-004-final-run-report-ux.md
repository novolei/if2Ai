# AWL-004 FinalRunReport UX Design

Status: draft
Owner: Frontend UI/UX Design
Pack: `AWL-004`

## Problem

The backend can emit `final_run_report`, and the frontend projection can store it,
but the chat surface still treats most endings as plain assistant text. Users need a
visible, consistent end state for successful work, blocked approvals, exhausted
budgets, provider failures, and partial results.

The UX goal is not to expose internals. It is to make the run understandable and
recoverable.

## UX Principles

- Show the current loop only where it helps explain behavior.
- Put the user's next action above diagnostic detail.
- Make failure states feel finished, not abandoned.
- Keep tool evidence scannable and collapsible.
- Use the runtime projection as the only frontend truth.

## Surfaces

### Run Inspector

The inspector is the live execution surface. It should show:

- loop kind,
- route reason,
- active/loaded skills,
- tool attempts timeline,
- approval state,
- final outcome.

Recommended visual structure:

- compact status header,
- skill chips,
- timeline rows,
- final report summary panel.

### Assistant Message Report

When a run ends, the transcript should show a small final report block attached to
the assistant turn. The block should contain:

- completed work,
- blocked or failed point,
- next step,
- retry suggestion when safe,
- evidence count / expandable details.

### Approval Blocked State

If the loop ends in `NeedsApproval`, the report should make the pending choice clear:

- operation,
- risk reason,
- key parameters,
- working directory,
- allow once / always allow / deny controls if the approval subsystem exposes them.

## State Mapping

| Backend outcome | User-facing state |
| --- | --- |
| `Completed` | Done |
| `NeedsApproval` | Waiting for approval |
| `NeedsUserInput` | Needs input |
| `FailedWithPlan` | Could not finish, plan available |
| `ExhaustedWithSummary` | Stopped after limits |

The UI text should be calm and specific. Avoid generic "something went wrong" copy.

## Projection Contract

The frontend reducer consumes these event types:

- `execution_mode_decision`
- `skill_resolution_snapshot`
- `tool_attempt_*` or existing tool timeline events
- `approval_*` events
- `final_run_report`

If a run has a final report but no tool attempts, the UI must still render a valid
summary.

## Responsive Behavior

Desktop:

- Inspector can live beside the transcript.
- Timeline rows may show tool name, status, duration, and short result.

Mobile:

- Inspector collapses into a sheet or inline disclosure.
- Timeline rows become single-column.
- Buttons and labels must not overflow their containers.

## Empty / Partial States

- No skills: show a neutral "No skill loaded" line only in inspector, not in the main
  assistant message.
- No evidence: hide the evidence section.
- Provider failure before text: render a failure report with retry guidance.
- Cancelled run: render what was attempted and whether retry is safe.

## Tests

- Success report renders completed work and evidence.
- Failure report renders failed point and retry guidance.
- Approval-blocked report renders the blocked action and approval state.
- Mobile layout does not overlap or truncate key labels.
