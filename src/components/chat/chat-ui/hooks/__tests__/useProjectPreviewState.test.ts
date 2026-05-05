import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../useProjectPreviewState.ts', import.meta.url), 'utf8')

test('useProjectPreviewState — declares the hook', () => {
  assert.match(source, /export function useProjectPreviewState\(/)
})

test('useProjectPreviewState — preserves the 900ms autosave debounce constant', () => {
  assert.match(source, /export const PREVIEW_AUTOSAVE_DELAY_MS = 900/)
})

test('useProjectPreviewState — openPreviewTab signature accepts a FilePreviewPayload', () => {
  assert.match(source, /openPreviewTab: \(preview: FilePreviewPayload\) => void/)
  assert.match(source, /closePreviewTab: \(path: string\) => void/)
  assert.match(source, /closePreviewPanel: \(\) => void/)
  assert.match(source, /refreshPreviewTab: \(absPath: string\) => Promise<void>/)
})
