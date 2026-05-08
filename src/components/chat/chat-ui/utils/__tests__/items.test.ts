import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  modelItems,
  modelLabelFor,
  permissionModeItems,
  permissionModeLabelFor,
  strengthItems,
  strengthLabelFor,
} from '@/components/chat/chat-ui/utils/items'

test('items — modelItems exposes the legacy default catalogue', () => {
  assert.equal(modelItems.length, 3)
  assert.deepEqual(
    modelItems.map((item) => item.value),
    ['gpt-5.4-mini', 'gpt-5.4', 'gpt-4.1'],
  )
})

test('items — strengthItems exposes 低/中/高 in legacy order', () => {
  assert.deepEqual(
    strengthItems.map((item) => item.value),
    ['low', 'mid', 'high'],
  )
  assert.deepEqual(
    strengthItems.map((item) => item.label),
    ['低', '中', '高'],
  )
})

test('items — permissionModeItems lists the three runtime modes', () => {
  assert.deepEqual(
    permissionModeItems.map((item) => item.value),
    ['dangerFullAccess', 'workspaceWrite', 'readOnly'],
  )
})

test('items — labelFor helpers fall back to legacy defaults', () => {
  assert.equal(modelLabelFor('gpt-5.4'), 'GPT-5.4')
  assert.equal(modelLabelFor('unknown-model'), 'GPT-5.4-Mini')
  assert.equal(strengthLabelFor('high'), '高')
  assert.equal(strengthLabelFor('xx'), '中')
  assert.equal(permissionModeLabelFor('readOnly'), '只读')
  assert.equal(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    permissionModeLabelFor('totally-bogus' as any),
    '完全访问权限',
  )
})
