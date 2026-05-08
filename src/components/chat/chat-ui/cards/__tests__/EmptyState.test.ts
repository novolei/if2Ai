import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../EmptyState.tsx', import.meta.url), 'utf8')

test('EmptyState — exports a single component that surfaces title + project label', () => {
  assert.match(source, /export function EmptyState\(/)
  assert.match(source, /sessionTitle: string/)
  assert.match(source, /projectLabel: string/)
  assert.match(source, /\{sessionTitle\}/)
  assert.match(source, /\{projectLabel\}/)
})

test('EmptyState — keeps the legacy hero copy + Sparkles icon', () => {
  assert.match(source, /import \{ Sparkles \} from 'lucide-react'/)
  assert.match(source, /当前工作区是/)
  assert.match(source, /min-h-\[55vh\]/)
})
