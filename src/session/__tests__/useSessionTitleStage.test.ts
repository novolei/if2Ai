// GF-03 PR-4 — coverage for src/session/useSessionTitleStage.ts.
//
// Mirrors the source-grep idiom used by the GF-03 PR-1 hook tests
// (`src/app-effects/__tests__/use*.test.ts`).  Behavioral assertions
// land on the precise code paths that drove the bug-bait sequences in
// the original `App.tsx` cluster: placeholder skip, non-placeholder
// auto-rename trigger, and rollback after a failed `renameSession`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../useSessionTitleStage.ts", import.meta.url),
  "utf8",
);

test("useSessionTitleStage: maybeAutoRenameSession bails when stage is locked or manual", () => {
  // The very first guard returns early for stages that the user (or a
  // prior auto-rename) already locked in — without this short-circuit
  // a placeholder-skip regression would re-write user titles.
  assert.match(
    source,
    /if \(titleState\.stage === "manual" \|\| titleState\.stage === "locked"\) return;/,
  );
  // And the placeholder branch — `getInitialSessionTitleState` defaults
  // to placeholder when no in-memory state exists, mirroring App.tsx.
  assert.match(
    source,
    /sessionTitleStates\[sessionId\] \?\?\s*getInitialSessionTitleState\(/,
  );
});

test("useSessionTitleStage: non-placeholder candidate triggers generated rename + lock", () => {
  // The rename trigger fires `syncGeneratedSessionTitle` then locks the
  // stage to `locked` with the canonical `MAX_AUTO_RENAME_COUNT`.
  assert.match(
    source,
    /syncGeneratedSessionTitle\(projectId, sessionId, initialCandidate\);/,
  );
  assert.match(source, /stage: "locked",\s*autoRenameCount: MAX_AUTO_RENAME_COUNT,/);
  // The placeholder gate is the precondition for triggering the
  // auto-rename branch — confirm it is the gate rather than a sibling.
  assert.match(
    source,
    /if \(titleState\.stage === "placeholder" && initialCandidate\) \{/,
  );
});

test("useSessionTitleStage: syncSessionTitle rolls back optimistic update on failure", () => {
  // The catch branch must restore both the conversation slice and the
  // project-sessions slice to `previousTitle`, plus surface a toast.
  assert.match(source, /void renameSession\(sessionId, nextTitle\)\.catch/);
  assert.match(
    source,
    /toast\.error\("重命名失败，已恢复原名称", \{ duration: 3000 \}\)/,
  );
  // Conversation rollback predicate: only restore if the title is
  // still the `nextTitle` we wrote (otherwise a manual rename raced us).
  assert.match(
    source,
    /if \(!c \|\| c\.title !== nextTitle\) return prev;\s*return \{ \.\.\.prev, \[sessionId\]: \{ \.\.\.c, title: previousTitle \} \};/,
  );
  // Project-sessions rollback maps the matching session back to the
  // previous title (guarded by id + title-equality).
  assert.match(
    source,
    /s\.id === sessionId && s\.title === nextTitle\s*\?\s*\{ \.\.\.s, title: previousTitle \}\s*:\s*s,/,
  );
});

test("useSessionTitleStage: handleRenameSession locks stage to manual before persisting", () => {
  // Sequencing matters: the stage must be locked first so any in-flight
  // auto-rename observes `manual` and bails (otherwise a race re-writes
  // the user's manual title).  The grep below preserves that ordering.
  assert.match(
    source,
    /setSessionTitleStates\(\(prev\) => \(\{[^)]*\[sessionId\]: \{\s*stage: "manual",\s*autoRenameCount: MAX_AUTO_RENAME_COUNT,\s*\},\s*\}\)\);\s*syncSessionTitle\(conv\.projectId, sessionId, newTitle\);/,
  );
});

test("useSessionTitleStage: ref-sync useEffect mirrors App.tsx L286-288", () => {
  assert.match(
    source,
    /useEffect\(\(\) => \{\s*sessionTitleStatesRef\.current = sessionTitleStates;\s*\}, \[sessionTitleStates, sessionTitleStatesRef\]\);/,
  );
});
