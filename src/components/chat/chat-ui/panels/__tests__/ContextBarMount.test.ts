import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ContextBarMount.tsx', import.meta.url), 'utf8')

test('ContextBarMount — exports the named mount wrapper declaration', () => {
  // Named export must survive the move so `ChatUI` can mount the bar.
  assert.match(source, /export function ContextBarMount\(/)
})
