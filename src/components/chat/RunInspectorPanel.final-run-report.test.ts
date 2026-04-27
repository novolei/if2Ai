import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const inspectorSource = readFileSync(
  new URL('./RunInspectorPanel.tsx', import.meta.url),
  'utf8',
)
const chatUiSource = readFileSync(
  new URL('../ui/chat-ui.tsx', import.meta.url),
  'utf8',
)

test('run inspector approval blocked panel exposes canonical approval details', () => {
  assert.match(inspectorSource, /const report = run\.finalRunReport/)
  assert.match(inspectorSource, /Operation: \{approval\.toolName\}/)
  assert.match(inspectorSource, /Mode: \{approval\.currentMode\}.*\{approval\.permissionMode\}/s)
  assert.match(inspectorSource, /State: pending user decision/)
  assert.match(inspectorSource, /Risk: \{approvalRisk\}/)
  assert.match(inspectorSource, /Workdir: \{approvalTool\.effectiveWorkdir\}/)
  assert.match(inspectorSource, /Params: \{formatToolArgs\(approvalTool\.toolArgs\)\}/)
})

test('run inspector hides empty report and skill sections', () => {
  assert.match(inspectorSource, /if \(visible\.length === 0\) return null/)
  assert.match(inspectorSource, /SkillChips title="Loaded skills"/)
  assert.match(inspectorSource, /SkillChips title="Blocked skills"/)
  assert.match(inspectorSource, /ReportSection title="Skill warnings"/)
})

test('chat tool renderer handles pending and failed statuses consistently', () => {
  assert.match(chatUiSource, /function isToolPendingStatus\(status: ChatToolStatus\): boolean/)
  assert.match(chatUiSource, /status === 'authorizing'/)
  assert.match(chatUiSource, /status === 'retrying'/)
  assert.match(chatUiSource, /function isToolFailureStatus\(status: ChatToolStatus\): boolean/)
  assert.match(chatUiSource, /status === 'failed'/)
  assert.match(chatUiSource, /status === 'blocked'/)
  assert.match(chatUiSource, /status === 'cancelled'/)
  assert.match(chatUiSource, /const isFailed = isToolFailureStatus\(status\)/)
  assert.match(chatUiSource, /status === 'completed' &&\s+Boolean\(writePath\)/)
})
