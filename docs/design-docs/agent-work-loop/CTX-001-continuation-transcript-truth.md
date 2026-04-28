# CTX-001 Continuation Transcript Truth

## Problem
The message "继续" carries almost no task semantics. If routing only reads the
latest text, a continuation of unfinished artifact work can be misclassified as
DirectAnswer.

## Design
TurnService remains the single entry point. During prompt preparation it replays
recent durable run-log entries for the session and extracts:

- the last tool-required terminal status,
- the failed/partial task outcome,
- resume availability and cursor,
- the original unfinished user goal when recorded.

These facts are folded into `WorkLoopRouteContext` and injected through a
`Continuation Context` prompt block when the selected loop requires tool
execution.

## Non-Goal
CTX-001 does not add a new session file, frontend slice, or shadow transcript.
