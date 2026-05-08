// GF-03 PR-2 — source-text + behaviour tests for
// `src/session/loadConversationHistory.ts`.
//
// Mirrors the PR-1 idiom (see `src/app-effects/__tests__/*.test.ts`).
// Behavioural tests would require mocking `@/api` + `@tauri-apps/api`;
// the source-text assertions instead pin the load-order, the
// reducer-loop branches, the replay-precedence rule, and the error
// degradation path so behaviour-equivalence with App.tsx L1456-1649
// is enforced at refactor time.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../loadConversationHistory.ts", import.meta.url),
  "utf8",
);

test("(a) replay-only path: when run-log entries exist, replayedMessages is preferred", () => {
  // Pulls history page entries through canonical replay reducer …
  assert.match(
    source,
    /historyPage && historyPage\.eventPage\.entries\.length > 0/,
  );
  assert.match(
    source,
    /replayRunLogEntriesToMessages\(\s*historyPage\.eventPage\.entries,\s*sessionId,\s*\)/,
  );
  // … with disableAnimation forced on so reload doesn't restart the
  // streaming animation.
  assert.match(source, /disableAnimation: true/);
});

test("(b) fullSession-only path: walks `fullSession.messages` blocks when no replay substance", () => {
  // Walks legacy `messages[].blocks[]` regardless, then chooses replay
  // only when it has assistant/tool entries.
  assert.match(source, /for \(const msg of fullSession\.messages\)/);
  assert.match(source, /for \(const block of msg\.blocks\)/);
  // Skips system messages (legacy compat).
  assert.match(source, /if \(msg\.role === "system"\)/);
});

test("(c) replay precedence: replayed wins only when it has assistant/tool entries", () => {
  // The `.some(...)` guard is the load-bearing predicate from App.tsx
  // L1594-1600; without it a replay containing only `user` rows would
  // overwrite the richer fullSession reducer output.
  assert.match(
    source,
    /replayedMessages\.some\(\s*\(message\) =>\s*message\.role === "assistant" \|\| message\.role === "tool",\s*\)/,
  );
  assert.match(source, /convertedMessages = replayedMessages/);
});

test("(d) tool_use + tool_result merge: upserts under one tool-call id", () => {
  // The `upsertToolMessage` helper folds tool_use → tool_result onto
  // a single Message keyed by tool-call id.  Identity (`id`) sticks to
  // the first message we pushed so React keys remain stable across
  // the merge.
  assert.match(source, /const upsertToolMessage = \(toolCallId: string/);
  assert.match(
    source,
    /id: convertedMessages\[existingIndex\]\.id,/,
  );
  // tool_use seeds with status "running" + policyDecision "prompt".
  assert.match(source, /toolStatus: "running"/);
  assert.match(source, /policyDecision: "prompt"/);
  // tool_result patches in completion + restores prior toolArgs.
  assert.match(source, /toolStatus: "completed"/);
  assert.match(source, /policyDecision: "allow"/);
  assert.match(
    source,
    /existingIndex !== undefined\s*\? convertedMessages\[existingIndex\]\?\.toolArgs/,
  );
});

test("(e) TodoWrite extraction: harvests todos when result is a TodoWrite payload", () => {
  // Detects TodoWrite by current block name OR previously-recorded
  // toolName (matches App.tsx L1553-1557 mirror).
  assert.match(
    source,
    /\(block\.tool_name \?\? ""\)\.trim\(\) === "TodoWrite"/,
  );
  assert.match(
    source,
    /convertedMessages\[existingIndex\]\?\.toolName\?\.trim\(\) ===\s*"TodoWrite"/,
  );
  // Harvested todos overwrite running tally only when extraction
  // actually returned a list.
  assert.match(source, /const nextTodos = extractTodosFromToolResult\(block\.output\)/);
  assert.match(source, /if \(nextTodos\) \{\s*recoveredTodos = nextTodos;/);
});

test("(f) error degradation: returns empty conversation seeded from sessionMeta on throw", () => {
  // The catch block must still resolve (never throw) and produce a
  // valid LoadedSessionHistory so the UI keeps rendering the title row.
  assert.match(source, /catch \(err\) \{\s*console\.error\("Failed to load session:"/);
  // Fallback title chain matches original (sessionMeta?.title || PLACEHOLDER).
  assert.match(
    source,
    /const fallbackTitle =\s*sessionMeta\?\.title \|\| PLACEHOLDER_SESSION_TITLE;/,
  );
  // Empty messages, fresh updatedAt, empty todos.
  assert.match(source, /messages: \[\],\s*updatedAt: new Date\(\),/);
  assert.match(source, /recoveredTodos: \[\]/);
});

test("module surface: exports the documented LoadedSessionHistory + loadConversationHistory", () => {
  assert.match(source, /export type LoadedSessionHistory = \{/);
  assert.match(
    source,
    /export async function loadConversationHistory\(args: \{/,
  );
  // Args contract used by App.tsx must stay stable.
  assert.match(source, /sessionId: string;/);
  assert.match(source, /projectId: string;/);
  assert.match(source, /sessionMeta: SessionMeta \| undefined;/);
  assert.match(source, /projectWorkdir: string \| undefined;/);
});
