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
// GF-01 PR-02 — tool-status helpers moved into the toolCallDisplay utils
// module. Read both sources so the legacy assertions can match the new
// home without losing coverage.
const toolCallBuildersSource = readFileSync(
  new URL('./chat-ui/utils/toolCallDisplay/builders.ts', import.meta.url),
  'utf8',
)
const telemetryDrawerSource = readFileSync(
  new URL('./TelemetryDrawer.tsx', import.meta.url),
  'utf8',
)
const chatWorkspaceSource = readFileSync(
  new URL('../../modules/chat/components/ChatWorkspace.tsx', import.meta.url),
  'utf8',
)
const mainShellSource = readFileSync(
  new URL('../../shell/MainShell.tsx', import.meta.url),
  'utf8',
)
const executionModePillSource = readFileSync(
  new URL('../../modules/execution-mode/ExecutionModePill.tsx', import.meta.url),
  'utf8',
)
const promptDiagnosticsSource = readFileSync(
  new URL('../../modules/prompt-diagnostics/components/PromptDiagnosticsPanel.tsx', import.meta.url),
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
  assert.match(inspectorSource, /<ReportSection\s+title="Skill warnings"/)
})

test('run inspector tolerates older final report snapshots with missing arrays', () => {
  assert.match(inspectorSource, /function safeStringList\(value: unknown\): string\[\]/)
  assert.match(inspectorSource, /Object\.values\(snapshot\.runs \?\? \{\}\)/)
  assert.match(inspectorSource, /Object\.values\(run\.toolCalls \?\? \{\}\)/)
  assert.match(inspectorSource, /const reportLoadedSkills = safeStringList\(report\?\.loadedSkills\)/)
  assert.match(inspectorSource, /const reportCompletedItems = safeStringList\(report\?\.completedItems\)/)
  assert.doesNotMatch(inspectorSource, /report\?\.loadedSkills\.length/)
  assert.doesNotMatch(inspectorSource, /report\.completedItems/)
})

test('run inspector exposes canonical work loop reason codes and tool policy', () => {
  assert.match(inspectorSource, /const workLoopReasons = safeStringList\(run\.workLoop\?\.reasonCodes\)/)
  assert.match(inspectorSource, /ReportSection title="Route reasons" items=\{workLoopReasons\}/)
  assert.match(inspectorSource, /toolDefinitionsVisibleLabel\(run\.workLoop\?\.loopKind\)/)
  assert.match(inspectorSource, /definitions visible/)
  assert.match(inspectorSource, /definitions hidden/)
})

test('run inspector is embedded in developer telemetry instead of floating over chat', () => {
  assert.match(inspectorSource, /variant\?: "floating" \| "embedded"/)
  assert.match(inspectorSource, /variant === "embedded" && "w-full shadow-sm"/)
  assert.match(telemetryDrawerSource, /<SectionHeader title="Run Inspector" \/>/)
  assert.match(telemetryDrawerSource, /<RunInspectorPanel sessionId=\{sessionId\} variant="embedded" \/>/)
  assert.doesNotMatch(chatWorkspaceSource, /<RunInspectorPanel/)
})

test('execution mode pill lives in developer telemetry instead of the app title chrome', () => {
  assert.match(telemetryDrawerSource, /<SectionHeader title="Execution Decision" \/>/)
  assert.match(telemetryDrawerSource, /<ExecutionModePill className="max-w-full" \/>/)
  assert.match(telemetryDrawerSource, /executionMode \?/)
  assert.doesNotMatch(mainShellSource, /<ExecutionModePill/)
  assert.match(executionModePillSource, /inline-flex max-w-full flex-wrap/)
  assert.match(executionModePillSource, /Array\.isArray\(projection\.reasonCodes\)/)
  assert.match(executionModePillSource, /MODE_LABEL\[projection\.executionMode\] \?\? projection\.executionMode/)
})

test('developer telemetry uses theme tokens for drawer chrome', () => {
  assert.match(telemetryDrawerSource, /bg-background text-foreground/)
  assert.match(telemetryDrawerSource, /border-border\/70/)
  assert.match(telemetryDrawerSource, /text-muted-foreground/)
  assert.doesNotMatch(telemetryDrawerSource, /bg-white|text-black|border-black|bg-black/)
})

test('developer telemetry tolerates older telemetry snapshots with missing counters', () => {
  assert.match(telemetryDrawerSource, /function numberValue\(value: unknown\): number/)
  assert.match(telemetryDrawerSource, /function recordValue\(value: unknown\): Record<string, number>/)
  assert.match(telemetryDrawerSource, /function stringArrayValue\(value: unknown\): string\[\]/)
  assert.match(telemetryDrawerSource, /stringArrayValue\(harness\.active_recordings\)\.includes\(sessionId\)/)
  assert.match(telemetryDrawerSource, /const toolCalls = telemetry \? recordValue\(telemetry\.tool_calls\) : \{\}/)
  assert.match(telemetryDrawerSource, /const turnsCompleted = numberValue\(telemetry\?\.turns_completed\)/)
  assert.match(telemetryDrawerSource, /class TelemetryDrawerErrorBoundary extends Component/)
  assert.match(telemetryDrawerSource, /Developer telemetry render failed/)
})

test('prompt diagnostics tolerates older snapshots with missing trace and arrays', () => {
  assert.match(promptDiagnosticsSource, /function shortenTraceId\(traceId: unknown\): string/)
  assert.match(promptDiagnosticsSource, /const id = stringValue\(traceId\)/)
  assert.match(promptDiagnosticsSource, /function safeLaneSummaries\(summary: PromptDiagnosticsSummary\)/)
  assert.match(promptDiagnosticsSource, /function safeActivationReasonCodes\(summary: PromptDiagnosticsSummary\)/)
  assert.match(promptDiagnosticsSource, /const laneSummaries = safeLaneSummaries\(summary\)/)
  assert.doesNotMatch(promptDiagnosticsSource, /traceId\.length/)
  assert.doesNotMatch(promptDiagnosticsSource, /summary\.activation_reason_codes\.length/)
})

test('chat tool renderer handles pending and failed statuses consistently', () => {
  // Helper definitions live in toolCallDisplay/builders.ts (GF-01 PR-02).
  assert.match(toolCallBuildersSource, /function isToolPendingStatus\(status: ChatToolStatus\): boolean/)
  assert.match(toolCallBuildersSource, /status === 'authorizing'/)
  assert.match(toolCallBuildersSource, /status === 'retrying'/)
  assert.match(toolCallBuildersSource, /function isToolFailureStatus\(status: ChatToolStatus\): boolean/)
  assert.match(toolCallBuildersSource, /status === 'failed'/)
  assert.match(toolCallBuildersSource, /status === 'blocked'/)
  assert.match(toolCallBuildersSource, /status === 'cancelled'/)
  // Caller wiring stays in chat-ui.tsx.
  assert.match(chatUiSource, /const isFailed = isToolFailureStatus\(status\)/)
  assert.match(chatUiSource, /status === 'completed' &&\s+Boolean\(writePath\)/)
})
