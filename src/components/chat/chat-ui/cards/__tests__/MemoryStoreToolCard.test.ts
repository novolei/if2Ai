import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../MemoryStoreToolCard.tsx', import.meta.url), 'utf8')

test('MemoryStoreToolCard — exports a named component delegating to MemoryWriteCard', () => {
  assert.match(source, /export function MemoryStoreToolCard\(/)
  assert.match(source, /import \{ MemoryWriteCard \}/)
  assert.match(source, /<MemoryWriteCard/)
})

test('MemoryStoreToolCard — preserves the legacy decision/reason fallback ladder', () => {
  assert.match(source, /POLICY_DENIED/)
  assert.match(source, /USER_APPROVAL_REQUIRED/)
  assert.match(source, /ALLOWED_BY_POLICY/)
  assert.match(source, /content_preview/)
})

test('MemoryStoreToolCard — defaults the scope to session when unspecified', () => {
  assert.match(source, /message\.memoryScope \?\? scopeFromArgs \?\? 'session'/)
})
