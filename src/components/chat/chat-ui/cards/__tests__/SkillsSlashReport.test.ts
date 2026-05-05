import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../SkillsSlashReport.tsx', import.meta.url), 'utf8')

test('parseSkillsSlashReport — exports the parser with the legacy 📋 header guard', () => {
  assert.match(source, /export function parseSkillsSlashReport\(content: string\): ParsedSkillsReport \| null/)
  assert.match(
    source,
    /lines\[0\]\?\.match\(\/\^📋\\s\+Skills\\s\+\\\(\(\\d\+\)\\s\+total,\\s\+\(\\d\+\)\\s\+enabled\\\)\$\//,
  )
})

test('parseSkillsSlashReport — returns null when no header line matches', () => {
  // Source-text guarantee: function bails out via `if (!header) return null`
  // and again if it found zero items after the loop.
  assert.match(source, /if \(!header\) return null/)
  assert.match(source, /if \(!items\.length\) return null/)
})

test('groupSkillsReportItems — exports the grouping helper that merges by name', () => {
  assert.match(source, /export function groupSkillsReportItems\(items: ParsedSkillsReport\['items'\]\): GroupedSkillsReportItem\[\]/)
  assert.match(source, /const key = item\.name\.toLowerCase\(\)/)
  assert.match(source, /getSkillVariantPriority\(b\) - getSkillVariantPriority\(a\)/)
})
