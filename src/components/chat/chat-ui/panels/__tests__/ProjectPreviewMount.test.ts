import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ProjectPreviewMount.tsx', import.meta.url), 'utf8')
const previewHookSource = readFileSync(
  new URL('../../hooks/useProjectPreviewState.ts', import.meta.url),
  'utf8',
)

test('ProjectPreviewMount — exports the named mount wrapper declaration', () => {
  // Named export must survive the move so `ChatUI` can mount the panel
  // without re-importing `@/components/ui/ProjectPreviewPanel` directly.
  assert.match(source, /export function ProjectPreviewMount\(/)
})

test('ProjectPreviewMount — 900ms autosave timer now lives in useProjectPreviewState (PR-08)', () => {
  // PR-07 deferred extraction of the preview state machine; PR-08
  // landed it in `useProjectPreviewState`. The 900ms
  // `PREVIEW_AUTOSAVE_DELAY_MS` debounce moved with it.
  assert.match(source, /Scope note \(deferred\)/)
  assert.match(previewHookSource, /export const PREVIEW_AUTOSAVE_DELAY_MS = 900/)
  assert.match(previewHookSource, /\}, PREVIEW_AUTOSAVE_DELAY_MS\)/)
})

test('ProjectPreviewMount — preserves dirty/draft/save state surface as props', () => {
  // The mount must keep its full props surface so the future hook drop-
  // in stays mechanical (zero call-site change inside `ChatUI`).
  assert.match(source, /drafts: Record<string, string>/)
  assert.match(source, /saveStates: Record<string, 'idle' \| 'saving' \| 'saved' \| 'error'>/)
  assert.match(source, /dirtyPaths: string\[\]/)
})
