import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../useComposerOverlays.ts', import.meta.url), 'utf8')

test('useComposerOverlays — declares the hook + both overlay state types', () => {
  assert.match(source, /export function useComposerOverlays\(\)/)
  assert.match(source, /export interface SlashOverlayState/)
  assert.match(source, /export interface AtOverlayState/)
})

test('useComposerOverlays — exposes both timer refs for unmount cleanup', () => {
  assert.match(source, /slashTimerRef: React\.MutableRefObject<ReturnType<typeof setTimeout> \| null>/)
  assert.match(source, /atTimerRef: React\.MutableRefObject<ReturnType<typeof setTimeout> \| null>/)
})
