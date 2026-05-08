import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../TodoPanelMount.tsx', import.meta.url), 'utf8')

test('TodoPanelMount — exports the named mount wrapper declaration', () => {
  // Named export must survive the move so `ChatUI` can mount the panel.
  assert.match(source, /export function TodoPanelMount\(/)
})
