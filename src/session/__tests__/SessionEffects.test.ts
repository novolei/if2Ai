// GF-03 PR-6 — source-text pins for `useSessionRuntime` (the hook in
// `SessionEffects.tsx`).  Mirrors the GF-03 test idiom: behaviour
// equivalence with App.tsx is enforced via load-bearing source
// fragments rather than a React renderer.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../SessionEffects.tsx", import.meta.url),
  "utf8",
);

test("(1) source-text presence: ER-03 TODO + canonical refs are owned here", () => {
  // Rapid session-switch race window is documented inline so future
  // readers know not to add ad-hoc guards before ER-03 ships.
  assert.match(source, /TODO\(ER-03\)/);
  assert.match(source, /runtimeProjectionStore\.swapSession/);
  // The three lifecycle refs migrated out of App.tsx into the hook.
  assert.match(source, /sessionLoadingRef = useRef<Record<string, boolean>>/);
  assert.match(source, /autoResumeAttemptsRef = useRef<Record<string, number>>/);
  assert.match(
    source,
    /attemptedAutoResumeCursorsRef = useRef<Set<string>>\(new Set\(\)\)/,
  );
  // sessionLoadingRef sync useEffect lives inside the hook now.
  assert.match(
    source,
    /sessionLoadingRef\.current = deps\.sessionLoading/,
  );
});

test("(2) useSessionRuntime export contract: returns the chat runtime API", () => {
  // Public hook + interface signatures.
  assert.match(source, /export function useSessionRuntime\(/);
  assert.match(source, /export interface SessionRuntimeApi/);
  // The six members the chat surface depends on.
  assert.match(source, /sendChatTurn:\s*\(/);
  assert.match(source, /stopAgentStream:\s*\(/);
  assert.match(source, /handleResumeFromCursor:\s*\(/);
  assert.match(source, /handleConversationUndo:\s*\(/);
  assert.match(source, /handleConversationRedo:\s*\(/);
  assert.match(source, /conversationUndoStatus:/);
  // Self-reference for the auto-resume loop is via a ref so the
  // outer useCallback dep list can stay empty.
  assert.match(source, /sendChatTurnRef = useRef</);
  assert.match(
    source,
    /resume:\s*\(text, opts\) => sendChatTurnRef\.current!\(text, opts\)/,
  );
});

test("(3) undo/redo wiring: confirm-on-streaming → stop → projection discard → reload", () => {
  // The undo/redo branch must:
  //   a) confirm before nuking an in-flight stream,
  //   b) call stopAgentStream first,
  //   c) dispatch projection_discard_session_runs + flush so stale
  //      runs disappear before the reload re-hydrates the conv,
  //   d) reload the session via reloadSession (forceReload: true on
  //      the App.tsx side),
  //   e) refresh the undo / redo status on success or failure.
  assert.match(source, /window\.confirm\(/);
  assert.match(source, /await stopAgentStream\(d\.activeSessionId\)/);
  assert.match(source, /sessionUndo\(d\.activeSessionId\)/);
  assert.match(source, /sessionRedo\(d\.activeSessionId\)/);
  assert.match(source, /kind: "projection_discard_session_runs"/);
  assert.match(source, /runtimeProjectionStore\.flush\(\)/);
  assert.match(source, /d\.reloadSession\(d\.activeProjectId, d\.activeSessionId\)/);
  assert.match(source, /"已撤销"/);
  assert.match(source, /"已重做"/);
  assert.match(source, /"撤销失败"/);
  assert.match(source, /"重做失败"/);
  assert.match(source, /void refreshConversationUndoStatus\(\)/);
});
