import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ThinkingBlock.tsx', import.meta.url), 'utf8')

test('ThinkingBlock — exports the named component with the legacy props', () => {
  assert.match(source, /export function ThinkingBlock\(/)
  assert.match(source, /thinking: string/)
  assert.match(source, /thinkingTime\?: number/)
  assert.match(source, /defaultOpen\?: boolean/)
})

test('ThinkingBlock — re-collapses when the thinking text changes', () => {
  assert.match(source, /React\.useEffect\([\s\S]*?\[defaultOpen, thinking\]/)
})

test('ThinkingBlock — renders the legacy "已完成思考" headline', () => {
  assert.match(source, /已完成思考/)
  assert.match(source, /import \{ formatDuration \} from '\.\.\/utils\/text'/)
})
