import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../FinalRunReportCard.tsx', import.meta.url), 'utf8')

test('FinalRunReportCard — exports the component with the AWL-004 contract', () => {
  assert.match(source, /export function FinalRunReportCard\(/)
  assert.match(source, /import type \{ FinalRunReport \} from '@\/transport\/contracts'/)
})

test('FinalRunReportCard — covers the four legacy outcome palettes', () => {
  assert.match(source, /title: 'Run completed'/)
  assert.match(source, /title: 'Waiting for approval'/)
  assert.match(source, /title: 'Stopped after limits'/)
  assert.match(source, /title: 'Could not finish'/)
})

test('FinalRunReportCard — humanizes runtime values with snake_case → space', () => {
  assert.match(source, /value\.replace\(\/_\/g, ' '\)/)
  assert.match(source, /return 'pending'/)
})
