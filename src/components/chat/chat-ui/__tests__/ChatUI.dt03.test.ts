/**
 * GF-01 PR-09 — DT-03 cutover invariants.
 *
 * Source-grep style tests guarding the data-flow change introduced
 * by PR-09:
 *
 *   - ChatUI now subscribes to the runtime projection store and
 *     the chat-store conversation slice directly.
 *   - The `messages: Message[]` prop is gone from `ChatUIProps`.
 *   - The `currentSessionId` snapshot field added by ER-03 is the
 *     scope guard that prevents cross-session leakage during a
 *     session swap.
 *   - `src/components/ui/chat-ui.tsx` is reduced to a re-export
 *     shim so the (many) child modules importing `Message` /
 *     `ComposerDropItem` from that path keep compiling.
 *   - `ChatWorkspace` no longer drills `messages={activeMessages}`
 *     into the `<ChatUI>` JSX call site (App.tsx still computes
 *     `activeMessages` for the title-stage and prompt-diagnostics
 *     surfaces — explicitly out of scope for PR-09).
 */

import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { test } from "node:test"

const chatUiSource = readFileSync(
  new URL("../ChatUI.tsx", import.meta.url),
  "utf8",
)
const shimSource = readFileSync(
  new URL("../../../ui/chat-ui.tsx", import.meta.url),
  "utf8",
)
const chatWorkspaceSource = readFileSync(
  new URL(
    "../../../../modules/chat/components/ChatWorkspace.tsx",
    import.meta.url,
  ),
  "utf8",
)

test("DT-03 (1) projection-first invariant — ChatUI reads its own messages", () => {
  assert.match(
    chatUiSource,
    /useRuntimeProjectionSelector/,
    "ChatUI.tsx must subscribe to the runtime projection store",
  )
  assert.match(
    chatUiSource,
    /projectConversationMessagesFromRuns\s*\(/,
    "ChatUI.tsx must call projectConversationMessagesFromRuns",
  )
  assert.match(
    chatUiSource,
    /useConversation\s*\(/,
    "ChatUI.tsx must read the chat-store user-anchor conversation slice",
  )
})

test("DT-03 (2) ChatUIProps does not declare a `messages` field", () => {
  // Locate the ChatUIProps interface block and assert the body
  // does not contain a `messages: Message[]` (or `readonly`) field.
  const propsBlock = chatUiSource.match(
    /interface ChatUIProps[\s\S]*?\n\}/,
  )?.[0]
  assert.ok(propsBlock, "ChatUIProps interface must be declared in ChatUI.tsx")
  assert.equal(
    propsBlock.match(/\bmessages\s*:\s*(readonly\s+)?Message\[\]/),
    null,
    "ChatUIProps must NOT contain `messages: Message[]` (or readonly variant)",
  )
})

test("DT-03 (3) ER-03 integration — ChatUI consults snapshot.currentSessionId", () => {
  assert.match(
    chatUiSource,
    /currentSessionId/,
    "ChatUI.tsx must reference the projection snapshot's currentSessionId field",
  )
  // The selector closure must read currentSessionId off the snapshot.
  assert.match(
    chatUiSource,
    /\bs\.currentSessionId\b/,
    "ChatUI.tsx must select s.currentSessionId from the projection snapshot",
  )
})

test("DT-03 (4) src/components/ui/chat-ui.tsx is now a thin re-export shim", () => {
  const lineCount = shimSource.split("\n").length
  assert.ok(
    lineCount <= 50,
    `chat-ui.tsx shim must be ≤ 50 LOC (actual: ${lineCount})`,
  )
  assert.match(
    shimSource,
    /export\s*\{[\s\S]*ChatUI[\s\S]*\}\s*from\s*["']@\/components\/chat\/chat-ui\/ChatUI["']/,
    "chat-ui.tsx must re-export ChatUI from @/components/chat/chat-ui/ChatUI",
  )
})

test("DT-03 (5) ChatWorkspace no longer prop-drills `messages={activeMessages}` into <ChatUI>", () => {
  // The God-component App.tsx still computes activeMessages for the
  // session-title-stage rename + latestPromptDiagnosticsSnapshot
  // (out of scope for PR-09).  What PR-09 *does* guarantee is the
  // <ChatUI> JSX call site no longer receives `messages=`.
  assert.equal(
    chatWorkspaceSource.match(/messages\s*=\s*\{\s*activeMessages\s*\}/),
    null,
    "ChatWorkspace.tsx must not forward `messages={activeMessages}` to <ChatUI>",
  )
})
