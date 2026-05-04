import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  formatDuration,
  formatShortTime,
  redactSensitiveText,
  summarizeThinkingText,
  truncateText,
} from '@/components/chat/chat-ui/utils/text'

test('text utils — formatDuration emits ms/s with legacy thresholds', () => {
  assert.equal(formatDuration(0), '0ms')
  assert.equal(formatDuration(999), '999ms')
  assert.equal(formatDuration(1000), '1.0s')
  assert.equal(formatDuration(2456), '2.5s')
})

test('text utils — formatShortTime produces HH:MM and tolerates invalid', () => {
  assert.equal(formatShortTime(new Date(NaN)), '')
  // sample at a fixed instant; locale formatter is stable across runs
  const formatted = formatShortTime(new Date('2026-05-05T03:07:00Z'))
  assert.match(formatted, /^\d{2}:\d{2}$/)
})

test('text utils — truncateText preserves short, ellipsises long', () => {
  assert.equal(truncateText('abc', 10), 'abc')
  assert.equal(truncateText('abcdefghij', 5), 'abcde…')
})

test('text utils — redactSensitiveText masks tokens and bearer headers', () => {
  assert.equal(
    redactSensitiveText('api_key = sk-12345 trailing'),
    'api_key=<redacted> trailing',
  )
  assert.equal(
    redactSensitiveText('Bearer abc.def-123'),
    'Bearer <redacted>',
  )
  assert.equal(redactSensitiveText(''), '')
})

test('text utils — summarizeThinkingText counts non-empty lines', () => {
  assert.equal(summarizeThinkingText(''), '已完成思考')
  assert.equal(summarizeThinkingText('one'), '已完成上下文判断')
  assert.equal(summarizeThinkingText('a\n\nb\nc'), '已完成上下文判断 · 3 段')
})
