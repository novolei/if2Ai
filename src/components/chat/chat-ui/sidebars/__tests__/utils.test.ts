import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  PROJECT_RAIL_MAX_WIDTH,
  PROJECT_RAIL_MIN_WIDTH,
  inferPreviewLanguage,
  sortRailDirectoryEntries,
} from '@/components/chat/chat-ui/sidebars/utils'
import type { DirectoryEntryPreview } from '@/lib/tauri'
// `import type` is erased at runtime so the tauri module is never loaded
// during tests; the runner only needs the literal helper exports above.

test('inferPreviewLanguage — maps known extensions and falls back to text', () => {
  // Markdown comes from extension OR from explicit kind; common code
  // extensions pass through verbatim; unknowns must fall back to `text`
  // so the highlighter never receives an undefined language tag.
  assert.equal(inferPreviewLanguage('a.md'), 'markdown')
  assert.equal(inferPreviewLanguage('a.txt', 'markdown'), 'markdown')
  assert.equal(inferPreviewLanguage('a.tsx'), 'tsx')
  assert.equal(inferPreviewLanguage('a.rs'), 'rs')
  assert.equal(inferPreviewLanguage('a.unknown'), 'text')
  assert.equal(inferPreviewLanguage('Cargo.toml'), 'text')
})

test('sortRailDirectoryEntries — folders sort before files regardless of mode', () => {
  // Folders-first invariant: even when a file is more recently modified
  // than every folder, every folder must still precede every file.
  const entries: DirectoryEntryPreview[] = [
    { kind: 'file', name: 'z.txt', path: '/p/z.txt', modified_ms: 999 } as DirectoryEntryPreview,
    { kind: 'folder', name: 'a', path: '/p/a', modified_ms: 1 } as DirectoryEntryPreview,
    { kind: 'folder', name: 'b', path: '/p/b', modified_ms: 2 } as DirectoryEntryPreview,
    { kind: 'file', name: 'a.txt', path: '/p/a.txt', modified_ms: 500 } as DirectoryEntryPreview,
  ]
  const byRecent = sortRailDirectoryEntries(entries, 'recent')
  assert.deepEqual(byRecent.map((e) => e.kind), ['folder', 'folder', 'file', 'file'])
  // Within folders, recent mode orders by modified_ms desc (b before a).
  assert.deepEqual(byRecent.map((e) => e.name), ['b', 'a', 'z.txt', 'a.txt'])
})

test('sortRailDirectoryEntries — name mode uses natural-order locale compare', () => {
  // Numeric natural order: `f10.txt` must come after `f2.txt`, not before
  // (which would happen under naive string compare).
  const entries: DirectoryEntryPreview[] = [
    { kind: 'file', name: 'f10.txt', path: '/p/f10.txt', modified_ms: 0 } as DirectoryEntryPreview,
    { kind: 'file', name: 'f2.txt', path: '/p/f2.txt', modified_ms: 0 } as DirectoryEntryPreview,
  ]
  const byName = sortRailDirectoryEntries(entries, 'name')
  assert.deepEqual(byName.map((e) => e.name), ['f2.txt', 'f10.txt'])
})

test('utils — width clamp constants are exposed verbatim from chat-ui.tsx', () => {
  // The drag-resize handle in `ProjectFilesRail` clamps against these
  // bounds; their numeric values are part of the rail's UX contract.
  assert.equal(PROJECT_RAIL_MIN_WIDTH, 140)
  assert.equal(PROJECT_RAIL_MAX_WIDTH, 300)
})
