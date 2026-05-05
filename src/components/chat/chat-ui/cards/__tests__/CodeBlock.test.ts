import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../CodeBlock.tsx', import.meta.url), 'utf8')

test('CodeBlock — renders the block code surface with a copy button', () => {
  assert.match(source, /export function CodeBlock\(/)
  assert.match(source, /aria-label=\{copied \? '已复制' : '复制代码'\}/)
  assert.match(source, /<pre className=/)
})

test('CodeBlock — short-circuits to the inline narrative/keyword fallbacks', () => {
  assert.match(source, /looksLikeKeywordLineBlock\(rawCode\)/)
  assert.match(source, /looksLikeNarrativeTextBlock\(rawCode\)/)
})
