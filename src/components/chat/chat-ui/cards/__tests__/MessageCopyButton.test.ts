import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../MessageCopyButton.tsx', import.meta.url), 'utf8')

test('MessageCopyButton — exports the named component with the legacy props', () => {
  assert.match(source, /export function MessageCopyButton\(/)
  assert.match(source, /side: 'left' \| 'right'/)
  assert.match(source, /aria-label=\{copied \? '已复制' : '复制消息'\}/)
})
