import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ProjectPreviewMount.tsx', import.meta.url), 'utf8')
const chatUiSource = readFileSync(
  new URL('../../../../ui/chat-ui.tsx', import.meta.url),
  'utf8',
)

test('ProjectPreviewMount — exports the named mount wrapper declaration', () => {
  // Named export must survive the move so `ChatUI` can mount the panel
  // without re-importing `@/components/ui/ProjectPreviewPanel` directly.
  assert.match(source, /export function ProjectPreviewMount\(/)
})

test('ProjectPreviewMount — documents the deferred 900ms autosave timer in chat-ui', () => {
  // The 900ms `PREVIEW_AUTOSAVE_DELAY_MS` debounce is intentionally
  // kept in `ChatUI` for PR-07 (extraction of the state machine into
  // `useProjectPreviewState` is deferred to PR-08). Both halves of that
  // contract must remain visible in source.
  assert.match(source, /Scope note \(deferred\)/)
  assert.match(source, /PREVIEW_AUTOSAVE_DELAY_MS/)
  assert.match(chatUiSource, /const PREVIEW_AUTOSAVE_DELAY_MS = 900/)
  assert.match(chatUiSource, /\}, PREVIEW_AUTOSAVE_DELAY_MS\)/)
})

test('ProjectPreviewMount — preserves dirty/draft/save state surface as props', () => {
  // The mount must keep its full props surface so the future hook drop-
  // in stays mechanical (zero call-site change inside `ChatUI`).
  assert.match(source, /drafts: Record<string, string>/)
  assert.match(source, /saveStates: Record<string, 'idle' \| 'saving' \| 'saved' \| 'error'>/)
  assert.match(source, /dirtyPaths: string\[\]/)
})
