# PROV-001 Provider Tool Call Compatibility Diagnostics Design

Pack: `PROV-001`

## Problem

Some models, observed with Kimi in the bubble-shooter turn, can emit text that
looks like a tool call:

`<function_calls><invoke name="text_editor">...`

That text is not a provider tool-call event. Executing it would be unsafe, but
silently treating the turn as normal prose makes the agent look stalled.

## Target Behavior

- Detect textual tool-call markup in assistant text.
- Record a sanitized compatibility diagnostic with provider id, model id, markup
  family, and whether real tool events were also observed.
- Never execute textual markup.
- Feed the diagnostic into `FinalRunReport` / Run Inspector as a warning.
- Preserve AWL-010: tool-required work without mutating tool evidence fails
  resumably instead of completing.

## Detection Families

- XML-like: `<function_calls>`, `<invoke name=...>`, `<parameter name=...>`.
- OpenAI-like leaked JSON: `"tool_calls": [...]` in plain text without a tool
  event.
- Anthropic-like leaked blocks: `<tool_use>` in plain text without a tool event.

Detection should store only a short family label and a redacted excerpt hash or
length. It must not persist full generated file contents.

## UX

Run Inspector should show a compact warning:

`Provider emitted textual tool-call markup; no real tool event was received.`

The final report should suggest retrying with a provider/model that supports
tool calls or switching the current provider compatibility mode when such a mode
exists.

## Non-Goals

PROV-001 is diagnostics and recovery guidance only. It must not parse and run
the fake markup, and it must not rewrite provider clients.
