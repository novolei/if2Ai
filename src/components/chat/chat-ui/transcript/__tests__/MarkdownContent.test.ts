import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../MarkdownContent.tsx', import.meta.url), 'utf8')

test('MarkdownContent — renders react-markdown with the legacy plugin set', () => {
  assert.match(source, /export const MarkdownContent = React\.memo/)
  assert.match(source, /<ReactMarkdown[\s\S]*remarkPlugins=\{\[remarkGfm\]\}[\s\S]*rehypePlugins=\{\[rehypeHighlight\]\}/)
})

test('MarkdownContent — keeps the module-scoped normalize cache (FIFO bound 300)', () => {
  assert.match(source, /const markdownNormalizeCache = new Map<number, \{ source: string; normalized: string \}>/)
  assert.match(source, /markdownNormalizeCache\.size > 300/)
})
