import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../useChatScroll.ts', import.meta.url), 'utf8')

test('useChatScroll — declares the hook + bottom-epsilon constant', () => {
  assert.match(source, /export function useChatScroll\(\)/)
  assert.match(source, /BOTTOM_EPSILON_PX = 120/)
})

test('useChatScroll — exposes both scrollRef and bottomRef in the return shape', () => {
  assert.match(source, /bottomRef: React\.RefObject<HTMLDivElement \| null>/)
  assert.match(source, /scrollRef: React\.RefObject<HTMLDivElement \| null>/)
  assert.match(source, /scrollToBottom: \(\) => void/)
})
