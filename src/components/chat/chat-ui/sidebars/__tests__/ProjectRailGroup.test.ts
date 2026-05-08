import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ProjectRailGroup.tsx', import.meta.url), 'utf8')

test('ProjectRailGroup — exports the section-header helper declaration', () => {
  // Pure presentational; the named export must survive the move so the
  // rail can group its sub-lists with stable React DevTools labels.
  assert.match(source, /export function ProjectRailGroup\(/)
})
