import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../RecoveryCard.tsx', import.meta.url), 'utf8')

test('RecoveryCard — exports the component with the legacy props', () => {
  assert.match(source, /export function RecoveryCard\(/)
  assert.match(source, /degradedReason\?: string/)
  assert.match(source, /resumeCursor\?: string/)
  assert.match(source, /isRecovering\?: boolean/)
  assert.match(source, /onResume\?: \(resumeCursor: string\) => void/)
})

test('RecoveryCard — renders the localized recovery copy and resume button label', () => {
  assert.match(source, /任务已部分完成/)
  assert.match(source, /可继续恢复/)
  assert.match(source, /继续未完成任务/)
})

test('RecoveryCard — keeps the degraded-reason keyword mapping', () => {
  assert.match(source, /network_timeout/)
  assert.match(source, /max_iterations_reached/)
  assert.match(source, /read_only_success_before_failure/)
})
