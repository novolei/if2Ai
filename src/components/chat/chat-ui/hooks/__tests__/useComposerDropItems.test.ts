import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../useComposerDropItems.ts', import.meta.url), 'utf8')

test('useComposerDropItems — declares the hook', () => {
  assert.match(source, /export function useComposerDropItems\(\)/)
})

test('useComposerDropItems — exposes add/remove/file-reference drop handlers', () => {
  assert.match(source, /addDropItem: \(item: ComposerDropItem\) => void/)
  assert.match(source, /removeDropItem: \(id: string\) => void/)
  assert.match(source, /handleFileReferenceDrop: \(item: ComposerDropItem\) => void/)
})
