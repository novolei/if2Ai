// GF-03 PR-6 — source-text + behavioural pins for `sendChatTurn`.
//
// Mirrors the source-grep idiom used by the rest of GF-03 (see
// `loadConversationHistory.test.ts`).  Behavioural tests would
// require mocking `@/api`, `@tauri-apps/api`, and the projection
// store; the pins below instead enforce the load-bearing branches
// that App.tsx pre-extraction had inlined so any drift surfaces at
// test-time rather than via a manual smoke run.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../sendChatTurn.ts", import.meta.url),
  "utf8",
);
const slashSource = readFileSync(
  new URL("../handleSlashCommand.ts", import.meta.url),
  "utf8",
);

test("(a) builtin slash command path: executeSlashCommand → static assistant card", () => {
  // Routed via handleSlashCommand; the builtin branch falls through
  // when resolveSkillSlash returns null and the message is not
  // /compact.
  assert.match(slashSource, /executeSlashCommand\(messageText, sessionId\)/);
  // Detects the slash token from the trimmed message — preserves
  // App.tsx's slashCommand assignment so the chat surface can render
  // the slash-tagged card.
  assert.match(slashSource, /slashToken\s*=\s*messageText\.trim\(\)\.split\(\/\\s\+\/, 1\)\[0\]/);
  // Sets sessionLoading=false up-front so the composer re-enables
  // immediately even when the IPC fails.
  assert.match(
    slashSource,
    /setSessionLoading\(\(prev\) => \(\{ \.\.\.prev, \[sessionId\]: false \}\)\);[\s\S]+?try \{[\s\S]+?executeSlashCommand/,
  );
});

test("(b) skill slash command path: resolveSkillSlash → startChatTurn with SKILL.md", () => {
  // resolveSkillSlash returns the SKILL.md invocation text; we route
  // it through the agent so the LLM activates the skill.
  assert.match(slashSource, /resolveSkillSlash\(messageText, deps\.cwd\)/);
  // Reuses the parent createAssistantMessage so id sequencing matches
  // the normal turn (assistant message seeded BEFORE startChatTurn,
  // streamId patched AFTER).
  assert.match(
    slashSource,
    /deps\.createAssistantMessage\(\);[\s\S]+?startChatTurn\(\{[\s\S]+?deps\.createAssistantMessage\(handle\.streamId\)/,
  );
  // Binds the run id into the projection store so subsequent
  // runtime_event payloads project to the correct session.
  assert.match(
    slashSource,
    /kind: "stream_run_bound",\s*runId: handle\.streamId,\s*sessionId/,
  );
});

test("(c) /compact path: chatCompactSession → 已压缩 N 条消息 assistant card", () => {
  // /compact must match exactly OR be a prefix with a trailing space
  // (preserves App.tsx behaviour for `/compact <hint>`).
  assert.match(
    slashSource,
    /trimmedSlash === "\/compact" \|\|\s*trimmedSlash\.startsWith\("\/compact "\)/,
  );
  // Imports chatCompactSession dynamically — keeps @/lib/tauri off
  // the hot path.
  assert.match(
    slashSource,
    /const \{ chatCompactSession \} = await import\("@\/lib\/tauri"\)/,
  );
  // Renders the same card text App.tsx did, including the freed-token
  // formatting.
  assert.match(slashSource, /已压缩/);
  assert.match(slashSource, /freedTokens > 0/);
  assert.match(slashSource, /上下文已是最新，无需压缩。/);
  assert.match(slashSource, /slashCommand: "\/compact"/);
});

test("(d) normal turn: assistant message lifecycle (seed → bind streamId → subscribe)", () => {
  // Two-phase createAssistantMessage — first call seeds the assistant
  // message; second call only patches streamId once the IPC returns.
  assert.match(
    source,
    /createAssistantMessage\(\);\s*const handle = await startChatTurn\(\{/,
  );
  assert.match(
    source,
    /createAssistantMessage\(handle\.streamId\);[\s\S]+?setStreamAbortHandles/,
  );
  // The subscribe callback re-enters the side-effect handler with
  // the unlisten so finishProjectedStream can drop the listener.
  assert.match(
    source,
    /handle\.subscribe\([\s\S]+?handleProjectedStreamSideEffect\(payload, unlisten\)/,
  );
});

test("(e) stream_complete cleanup: drop loading + abort handle, refresh sessions, clear isRecovering", () => {
  // finishProjectedStream is the canonical cleanup path.
  assert.match(source, /const finishProjectedStream =/);
  // Unsets sessionLoading, drops the abort handle, and refreshes the
  // project sessions list (so the side-bar count updates).
  assert.match(
    source,
    /setSessionLoading\(\(prev\) => \(\{ \.\.\.prev, \[sessionId\]: false \}\)\)/,
  );
  assert.match(source, /\{ \[sessionId\]: _removed, \.\.\.rest \}/);
  assert.match(source, /refreshProjectSessions\(conv\.projectId\)/);
  // Clears isRecovering on every message so the partial-success
  // banner disappears once a new stream completes.
  assert.match(source, /msg\.isRecovering \? \{ \.\.\.msg, isRecovering: false \} : msg/);
  // stream_complete also resets the auto-resume bookkeeping.
  assert.match(
    source,
    /payload\.event_type === "stream_complete"[\s\S]+?autoResumeAttemptsRef\.current\[sessionId\] = 0[\s\S]+?attemptedAutoResumeCursorsRef\.current\.clear\(\)/,
  );
});

test("(f) stream_error + auto resume: partial_success → setTimeout(80) → resume(...)", () => {
  // The partial-success branch only fires when resume_available + a
  // resume_cursor are present.
  assert.match(
    source,
    /taskOutcome === "partial_success" &&\s*resumeAvailable &&\s*resumeCursor/,
  );
  // Caps the auto-resume attempts at 2 and dedupes by cursorKey so a
  // single failing cursor cannot loop forever.
  assert.match(source, /attemptCount < 2/);
  assert.match(source, /attemptedAutoResumeCursorsRef\.current\.add\(cursorKey\)/);
  // Recurses through deps.resume (self-reference owned by the hook).
  assert.match(
    source,
    /window\.setTimeout\(\(\) => \{[\s\S]+?deps\.resume\(buildResumePrompt\(resumeCursor\), \{[\s\S]+?isInternalResume: true,[\s\S]+?resumeCursor,[\s\S]+?\}\);[\s\S]+?\}, 80\)/,
  );
});

test("(g) startChatTurn throw → error message + sessionLoading=false", () => {
  // The catch block patches the existing assistant message when one
  // exists, otherwise creates a new error-only assistant message.
  assert.match(source, /catch \(err\) \{[\s\S]+?startAgentStream error:/);
  assert.match(source, /isError: true/);
  assert.match(source, /toolArgs: \{ rawError: errorMessage \}/);
  // Loading flag must be cleared even on the failure path.
  assert.match(
    source,
    /catch[\s\S]+?setSessionLoading\(\(prev\) => \(\{ \.\.\.prev, \[sessionId\]: false \}\)\)/,
  );
  // refreshProjectSessions also runs on failure so side-bar counts
  // stay in sync.
  assert.match(
    source,
    /catch[\s\S]+?refreshProjectSessions\(conv\.projectId\)/,
  );
});
