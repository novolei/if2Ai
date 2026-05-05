import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../useProjectRailState.ts', import.meta.url), 'utf8')

test('useProjectRailState — declares the hook', () => {
  assert.match(source, /export function useProjectRailState\(/)
  assert.match(source, /PROJECT_RAIL_WIDTH_STORAGE_KEY = 'projectRailWidthV1'/)
  assert.match(source, /PROJECT_RAIL_NOTICE_DURATION_MS = 2800/)
})

test('useProjectRailState — exposes a width slot persisted to localStorage', () => {
  assert.match(source, /projectRailWidth: number/)
  assert.match(source, /window\.localStorage\.setItem\(PROJECT_RAIL_WIDTH_STORAGE_KEY/)
  assert.match(source, /PROJECT_RAIL_DEFAULT_WIDTH = 200/)
})
