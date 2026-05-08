import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ProjectRailFilePreview.tsx', import.meta.url), 'utf8')

test('ProjectRailFilePreview — exports the named preview component', () => {
  // Named export must survive the move so the rail can mount it.
  assert.match(source, /export function ProjectRailFilePreview\(/)
})

test('ProjectRailFilePreview — preserves data-URL iframe + image branches', () => {
  // The kind-aware preview body must keep its iframe / image / markdown
  // dispatch — losing any branch would silently break a file kind.
  assert.match(source, /preview\.kind === 'image' && dataUrl/)
  assert.match(source, /preview\.kind === 'pdf' && dataUrl/)
  assert.match(source, /<iframe title=\{preview\.name\} src=\{dataUrl\}/)
  assert.match(source, /data:\$\{preview\.mime_type\};base64,\$\{preview\.data_base64\}/)
})
