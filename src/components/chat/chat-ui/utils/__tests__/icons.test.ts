import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../icons.tsx', import.meta.url), 'utf8')

test('icons — exposes all four font/density glyph components', () => {
  assert.match(source, /export function FontSansIcon\(/)
  assert.match(source, /export function FontSerifIcon\(/)
  assert.match(source, /export function DensityCompactIcon\(/)
  assert.match(source, /export function DensityComfortableIcon\(/)
})

test('icons — preserves legacy "Aa" glyph + density paths', () => {
  // Two SVGs render the literal "Aa" text (font preview).
  const aaCount = source.match(/>\s*Aa\s*</g)?.length ?? 0
  assert.equal(aaCount, 2, 'expected exactly two Aa glyphs')
  // Two density variants encode 5/8/11 vs 4/8/12 horizontal lines.
  assert.match(source, /M3 5h10M3 8h10M3 11h10/)
  assert.match(source, /M3 4h10M3 8h10M3 12h10/)
})
