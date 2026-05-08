import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ChatMessage.tsx', import.meta.url), 'utf8')

test('ChatMessage — exports the memoised component declaration', () => {
  // Memo wrapper + named factory must both survive the move so React
  // DevTools still labels the component as `ChatMessage`.
  assert.match(source, /export const ChatMessage = React\.memo\(function ChatMessage\(/)
})

test('ChatMessage — branches between user bubble and assistant body', () => {
  // The user vs assistant role split is the load-bearing render
  // structure: user gets the right-aligned secondary bubble; assistant
  // gets the full body (thinking / status / markdown / chips).
  assert.match(source, /const isUser = message\.role === 'user'/)
  assert.match(source, /isUser \? \(/)
  assert.match(source, /bg-secondary px-3 text-foreground\/80/)
  assert.match(source, /<MarkdownContent/)
})

test('ChatMessage — dispatches tool messages to ToolCallCard', () => {
  // Tool-role messages must short-circuit to the dedicated tool card so
  // the WriteToolDiffCard wrapper + status glyph stay reachable.
  assert.match(source, /import \{ ToolCallCard \} from "@\/components\/chat\/chat-ui\/cards\/ToolCallCard"/)
  assert.match(source, /if \(isTool\) \{[\s\S]*?<ToolCallCard message=\{message\} defaultWorkdir=\{defaultWorkdir\} \/>/)
})
