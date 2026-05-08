import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ToolCallCard.tsx', import.meta.url), 'utf8')

test('ToolCallCard — exports the card and a backwards-compat alias', () => {
  assert.match(source, /export function ToolCallCard\(/)
  assert.match(source, /export \{ ToolCallCard as ToolCallMessage \}/)
})

test('ToolCallCard — renders the pending/in-progress status glyph slot', () => {
  // Status glyph is driven by `normalizeToolStatus` and rendered through
  // `<ToolStatusGlyph status={status} />`; both must remain wired so that
  // pending tool calls keep their loading affordance.
  assert.match(source, /normalizeToolStatus\(message\)/)
  assert.match(source, /<ToolStatusGlyph status=\{status\} \/>/)
})

test('ToolCallCard — surfaces the failure render path with the rose palette', () => {
  // Failure marker text + rose-tinted styling guard the "执行失败" render
  // path that the legacy ToolCallMessage relied on.
  assert.match(source, /执行失败：\$\{display\.title\}/)
  assert.match(source, /text-rose-500\/72/)
})
