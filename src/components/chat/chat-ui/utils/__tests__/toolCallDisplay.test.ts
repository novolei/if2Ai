import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  buildToolCallDisplay,
  isToolFailureStatus,
  isToolPendingStatus,
  normalizeToolStatus,
  pickToolHeadline,
  pickToolString,
  shortenMiddle,
  tryParseJson,
} from '@/components/chat/chat-ui/utils/toolCallDisplay/builders'

const baseMessage = {
  id: 'm1',
  role: 'tool' as const,
  content: '',
  timestamp: new Date(0),
}

test('toolCallDisplay — normalizeToolStatus maps queued/running/completed/failed/error correctly', () => {
  // explicit toolStatus pass-through
  assert.equal(
    normalizeToolStatus({ ...baseMessage, toolStatus: 'queued' } as any),
    'queued',
  )
  assert.equal(
    normalizeToolStatus({ ...baseMessage, toolStatus: 'running' } as any),
    'running',
  )
  // isError flag → 'error'
  assert.equal(
    normalizeToolStatus({ ...baseMessage, isError: true } as any),
    'error',
  )
  // non-empty content + no status → 'completed'
  assert.equal(
    normalizeToolStatus({ ...baseMessage, content: 'ok' } as any),
    'completed',
  )
  // empty content + no status → 'running'
  assert.equal(
    normalizeToolStatus({ ...baseMessage } as any),
    'running',
  )

  assert.equal(isToolPendingStatus('running'), true)
  assert.equal(isToolPendingStatus('queued'), true)
  assert.equal(isToolPendingStatus('completed'), false)
  assert.equal(isToolFailureStatus('failed'), true)
  assert.equal(isToolFailureStatus('error'), true)
  assert.equal(isToolFailureStatus('completed'), false)
})

test('toolCallDisplay — buildToolCallDisplay golden fixture for read_file', () => {
  // Content is structured-looking so pickToolHeadline routes to the
  // path-based "查看了" branch instead of returning the content preview.
  const display = buildToolCallDisplay(
    {
      ...baseMessage,
      toolName: 'read_file',
      toolArgs: { path: '/tmp/example/foo.txt' },
      content: '{"exit_code":0,"stdout":"hi","stderr":""}',
    } as any,
    undefined,
    'completed',
  )
  // Structured JSON with stdout → summarizeToolResult yields "执行完成：hi"
  // and pickToolHeadline routes through the structured-result branch.
  assert.equal(display.title, '执行完成：hi')
  assert.ok(display.details.length >= 1)
  // first detail line carries the composite "命令/工具 · 结果" summary
  assert.ok(
    display.details[0].includes('read_file') || display.details[0].includes('结果'),
    `unexpected details[0]: ${display.details[0]}`,
  )
  assert.equal(display.diagnosticCopyText, null)
  assert.ok(display.copyText.length > 0)
})

test('toolCallDisplay — pickToolHeadline truncates declared title past 64 chars', () => {
  const long = 'a'.repeat(80)
  const headline = pickToolHeadline(
    'shell',
    'completed',
    { title: long },
    null,
    null,
    null,
    null,
    null,
    '',
    '',
    undefined,
  )
  // truncateText(80, 64) → 64 chars + '…'
  assert.equal(headline.length, 65)
  assert.ok(headline.endsWith('…'))
})

test('toolCallDisplay — tryParseJson parses valid + returns null on garbage', () => {
  assert.deepEqual(tryParseJson('{"a":1}'), { a: 1 })
  assert.deepEqual(tryParseJson('[1,2,3]'), [1, 2, 3])
  assert.equal(tryParseJson('{not json'), null)
  assert.equal(tryParseJson(''), null)
})

test('toolCallDisplay — pickToolString returns first non-empty string match', () => {
  const args = { a: '', b: '   ', c: 'hello', d: 'later' }
  assert.equal(pickToolString(args, ['a', 'b', 'c', 'd']), 'hello')
  assert.equal(pickToolString({}, ['a']), null)
  assert.equal(pickToolString({ a: 42 }, ['a']), null)
})

test('toolCallDisplay — shortenMiddle preserves short, ellipsises long', () => {
  assert.equal(shortenMiddle('short', 20), 'short')
  const out = shortenMiddle('abcdefghijklmnopqrstuvwxyz', 12)
  assert.ok(out.includes('…'))
  assert.ok(out.length <= 'abcdefgh…stuvwxyz'.length + 2)
})
