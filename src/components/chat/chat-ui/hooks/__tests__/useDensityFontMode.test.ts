import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../useDensityFontMode.ts', import.meta.url), 'utf8')

test('useDensityFontMode — declares the hook + Density/Font types', () => {
  assert.match(source, /export function useDensityFontMode\(/)
  assert.match(source, /export type DensityMode = 'comfortable' \| 'compact'/)
  assert.match(source, /export type FontMode = 'sans' \| 'serif'/)
})

test('useDensityFontMode — preserves the original localStorage keys', () => {
  assert.match(source, /CHAT_DENSITY_MODE_STORAGE_KEY = 'chatDensityModeV2'/)
  assert.match(source, /CHAT_FONT_MODE_STORAGE_KEY = 'chatFontModeV2'/)
})
