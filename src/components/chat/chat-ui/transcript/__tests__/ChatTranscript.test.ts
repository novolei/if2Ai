import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ChatTranscript.tsx', import.meta.url), 'utf8')

test('ChatTranscript — exports the memoised component declaration', () => {
  // Memo wrapper + named factory must both survive the move so React
  // DevTools still labels the component as `ChatTranscript`.
  assert.match(source, /export const ChatTranscript = React\.memo\(function ChatTranscript\(/)
})

test('ChatTranscript — renders both the virtual list and direct map branches against the messages prop', () => {
  // The virtual switch threshold must remain wired so long sessions opt
  // into virtualisation; both render branches must call ChatMessage so
  // the memoised single-message shell stays the leaf renderer.
  assert.match(source, /messages\.length >= VIRTUAL_LIST_THRESHOLD/)
  assert.match(source, /<VirtualMessageList/)
  assert.match(source, /messages\.map\(\(msg\) =>/)
  assert.match(source, /<ChatMessage/)
})

test('ChatTranscript — derives primaryThinkingMessageIds from the messages prop', () => {
  // The "first thinking block per turn" derivation must keep its
  // user→assistant reset semantics; the memo-key passes the resulting
  // Set down to ChatMessage as `isPrimaryThinkingMessage`.
  assert.match(source, /const primaryThinkingMessageIds = React\.useMemo/)
  assert.match(source, /seenThinkingThisTurn = false/)
  assert.match(source, /isPrimaryThinkingMessage=\{primaryThinkingMessageIds\.has\(msg\.id\)\}/)
})
