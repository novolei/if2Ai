# AWL-011 Tool Intent Nudge & Completion Evidence Invariant

## Problem
Some providers produce natural-language intent such as "I'll use bash to write
the file" without emitting a structured tool call. For artifact work this is a
runtime failure, not a completed answer.

## Design
TurnService detects assistant text that announces external action. When tools
are visible, the next iteration injects an agent-loop control message and forces
provider `tool_choice` to `Any`. If the model still does not produce a real
tool call, the existing AWL finalization path returns a resumable final report.

## Invariant
Artifact creation/modification only counts as completed when a successful
mutating tool result exists. Failed `ToolUse` attempts, prose, or textual fake
tool markup do not satisfy the invariant.
