import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../LoadingIndicator.tsx', import.meta.url), 'utf8')

test('LoadingIndicator — exports a parameterless component', () => {
  assert.match(source, /export function LoadingIndicator\(\)/)
})

test('LoadingIndicator — preserves the legacy WaveDots configuration', () => {
  assert.match(source, /import \{ WaveDotsAnimation \}/)
  assert.match(source, /amplitude=\{11\.04\}/)
  assert.match(source, /count=\{6\}/)
  assert.match(source, /horizontalStretch=\{1\.10625\}/)
  assert.match(source, /topStartColor="#fb923c"/)
})
