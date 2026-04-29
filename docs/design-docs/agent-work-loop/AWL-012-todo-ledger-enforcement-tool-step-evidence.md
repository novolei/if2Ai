# AWL-012 Todo Ledger Enforcement & Tool-Step Evidence

## Problem
`TodoWrite` was visible in the UI, but the runtime did not enforce the ledger.
This allowed a run to show a multi-step plan, get stuck on a later tool call,
and still drift toward a completed-looking state. `TodoWrite` also counted as a
mutating-tool success, which made the evidence invariant too weak: updating the
plan is not the same as creating the requested artifact.

## Design
The streaming loop now treats TodoWrite as a durable control ledger:

- Todo state is loaded from the same canonical store used by the `TodoWrite`
  tool.
- Tool-required turns with active/pending todos receive a prompt contribution
  that names the current ledger and requires tool-backed step progression.
- A successful non-`TodoWrite` mutation injects a follow-up control message that
  asks the model to reconcile the ledger before continuing.
- Finalization checks active/pending todos. On natural model stops, unfinished
  todos become `todo_ledger_incomplete`, which resolves as failed/resumable and
  appears in `FinalRunReport`.
- Specific terminal failures, including invalid tool args and approval blocks,
  keep their original status; todo evidence is added as diagnostics instead of
  hiding the primary cause.

## Invariant
Todo ledger progress is necessary but not sufficient evidence. Artifact work is
complete only when a relevant non-`TodoWrite` tool succeeds and the durable todo
ledger has no active or pending steps left.
