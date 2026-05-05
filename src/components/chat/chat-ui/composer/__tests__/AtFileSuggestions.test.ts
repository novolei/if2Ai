import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../AtFileSuggestions.tsx', import.meta.url), 'utf8')

test('AtFileSuggestions — exports a function component with typed props', () => {
  assert.match(source, /export function AtFileSuggestions\(/)
  assert.match(source, /export interface AtFileSuggestionsProps/)
})

test('AtFileSuggestions — wires onNavigateInto / onNavigateUp folder handlers', () => {
  assert.match(source, /onNavigateInto\?: \(entry: DirectoryEntryPreview\) => void/)
  assert.match(source, /onNavigateUp\?: \(\) => void/)
  assert.match(source, /onClick=\{\(\) => onNavigateUp\(\)\}/)
  assert.match(source, /if \(isFolder && onNavigateInto\) \{\s*onNavigateInto\(entry\)/)
})
