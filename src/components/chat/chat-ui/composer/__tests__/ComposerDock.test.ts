import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ComposerDock.tsx', import.meta.url), 'utf8')
const propsSource = readFileSync(
  new URL('../ComposerDock.props.ts', import.meta.url),
  'utf8',
)

test('ComposerDock — exports a memoised component with the legacy prop interface', () => {
  assert.match(source, /export const ComposerDock = React\.memo\(function ComposerDock\(/)
  assert.match(source, /export type \{ ComposerDockProps \}/)
  assert.match(propsSource, /export interface ComposerDockProps/)
})

test('ComposerDock — forwards textareaRef to the Textarea ref prop', () => {
  assert.match(propsSource, /textareaRef: React\.RefObject<HTMLTextAreaElement \| null>/)
  assert.match(source, /<Textarea\s+ref=\{textareaRef\}/)
})

test('ComposerDock — wires send/stop button to onSubmit / onStop', () => {
  assert.match(source, /if \(isLoading && onStop\) \{\s*onStop\(\)\s*\} else if \(!isLoading\) \{\s*onSubmit\(\)/)
  assert.match(source, /aria-label=\{isLoading \? '停止生成' : '发送消息'\}/)
})
