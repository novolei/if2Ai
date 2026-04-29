// UI-001..006 — Source-text invariants for the EvolutionDevDrawer +
// the 5 sub-panel components. Mirrors the project's existing test
// style (e.g. RunInspectorPanel.final-run-report.test.ts): we
// inspect the .tsx source rather than mounting React, keeping the
// suite zero-dep on JSDOM / RTL.

import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const drawerSource = readFileSync(
  new URL('./EvolutionDevDrawer.tsx', import.meta.url),
  'utf8',
)
const daemonSource = readFileSync(
  new URL('./DaemonHealthDashboard.tsx', import.meta.url),
  'utf8',
)
const compressionSource = readFileSync(
  new URL('./CompressionHistoryTable.tsx', import.meta.url),
  'utf8',
)
const skillsSource = readFileSync(
  new URL('./SkillSedimentationTimeline.tsx', import.meta.url),
  'utf8',
)
const selfEditSource = readFileSync(
  new URL('./SelfEditPanel.tsx', import.meta.url),
  'utf8',
)
const miscSource = readFileSync(
  new URL('./EvolutionMiscPanel.tsx', import.meta.url),
  'utf8',
)
const telemetryDrawerSource = readFileSync(
  new URL('../TelemetryDrawer.tsx', import.meta.url),
  'utf8',
)

// ── UI-001 ───────────────────────────────────────────────────────────────────

test('drawer_hidden_by_default — devMode gate returns null when off', () => {
  assert.match(drawerSource, /if \(!enabled\) return null/)
  assert.match(drawerSource, /IF2AI_DEV_EVOLUTION_UI/)
})

test('drawer_shows_six_tabs — EVOLUTION_TABS lists 5 tab specs (5 panels covering 6 slices)', () => {
  // 6 evolution slices are split across 5 panels per the architect's
  // condensation (UI-006 fuses 4 misc slices into one panel).
  const tabMatches = drawerSource.match(/\{ id: '\w+', label:/g) ?? []
  assert.equal(tabMatches.length, 5, `expected 5 tab specs, got ${tabMatches.length}`)
})

test('drawer_mounted_in_telemetry_drawer — TelemetryDrawer imports + renders EvolutionDevDrawer', () => {
  assert.match(telemetryDrawerSource, /import \{ EvolutionDevDrawer \} from/)
  assert.match(telemetryDrawerSource, /<EvolutionDevDrawer \/>/)
})

test('tab_switch_updates_state — useState<EvolutionTabId> + setActiveTab on click', () => {
  assert.match(drawerSource, /useState<EvolutionTabId>/)
  assert.match(drawerSource, /onClick=\{\(\) => setActiveTab\(tab\.id\)\}/)
})

test('no_ipc_import_in_drawer — no @tauri / invoke imports in any evolution component', () => {
  for (const [name, src] of [
    ['EvolutionDevDrawer', drawerSource],
    ['DaemonHealthDashboard', daemonSource],
    ['CompressionHistoryTable', compressionSource],
    ['SkillSedimentationTimeline', skillsSource],
    ['SelfEditPanel', selfEditSource],
    ['EvolutionMiscPanel', miscSource],
  ] as const) {
    assert.doesNotMatch(src, /from '@tauri/, `${name} must not import from @tauri-*`)
    assert.doesNotMatch(src, /\binvoke\(/, `${name} must not call invoke()`)
  }
})

// ── UI-002 ───────────────────────────────────────────────────────────────────

test('daemon_dashboard_subscribes_to_daemon_health_slice', () => {
  assert.match(daemonSource, /useEvolutionEventSelector\(\(s\) => s\.daemon_health\)/)
  assert.match(daemonSource, /data-testid="daemon-empty"/)
  assert.match(daemonSource, /data-testid="daemon-table"/)
})

// ── UI-003 ───────────────────────────────────────────────────────────────────

test('compression_table_subscribes_to_compression_event_slice', () => {
  assert.match(compressionSource, /useEvolutionEventSelector\(\(s\) => s\.compression_event\)/)
  assert.match(compressionSource, /Kept tokens|keptTokens/)
  assert.match(compressionSource, /passthrough/)
})

// ── UI-004 ───────────────────────────────────────────────────────────────────

test('skills_timeline_subscribes_to_skill_sedimented_slice', () => {
  assert.match(skillsSource, /useEvolutionEventSelector\(\(s\) => s\.skill_sedimented\)/)
  assert.match(skillsSource, /toolSequence/)
  assert.match(skillsSource, /sourceTurns/)
})

// ── UI-005 ───────────────────────────────────────────────────────────────────

test('self_edit_panel_subscribes_to_proposal_and_decision_slices', () => {
  assert.match(selfEditSource, /useEvolutionEventSelector\(\(s\) => s\.self_edit_proposal\)/)
  assert.match(selfEditSource, /useEvolutionEventSelector\(\(s\) => s\.verification_decision\)/)
  assert.match(selfEditSource, /data-testid="self-edit-tab-proposals"/)
  assert.match(selfEditSource, /data-testid="self-edit-tab-verdicts"/)
})

// ── UI-006 ───────────────────────────────────────────────────────────────────

test('misc_panel_subscribes_to_four_remaining_slices', () => {
  for (const slice of [
    'browser_health',
    'domain_knowledge',
    'checkpoint_updated',
    'content_simplified',
  ]) {
    const re = new RegExp(`useEvolutionEventSelector\\(\\(s\\) => s\\.${slice}\\)`)
    assert.match(miscSource, re, `EvolutionMiscPanel must subscribe to ${slice}`)
  }
})
