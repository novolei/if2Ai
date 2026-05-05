import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ProjectFilesRail.tsx', import.meta.url), 'utf8')

test('ProjectFilesRail — exports the memoised component declaration', () => {
  // Memo wrapper + named factory must survive the move so React DevTools
  // still labels the component as `ProjectFilesRail`.
  assert.match(source, /export const ProjectFilesRail = React\.memo\(function ProjectFilesRail\(/)
})

test('ProjectFilesRail — preserves drag-resize props + width clamp wiring', () => {
  // The pointer-capture resize closure must keep its width clamp against
  // the `PROJECT_RAIL_MIN_WIDTH` / `PROJECT_RAIL_MAX_WIDTH` bounds so the
  // sidebar can never escape its sizing envelope.
  assert.match(source, /onWidthChange: React\.Dispatch<React\.SetStateAction<number>>/)
  assert.match(source, /onPointerDown=\{startResize\}/)
  assert.match(source, /Math\.min\(PROJECT_RAIL_MAX_WIDTH, Math\.max\(PROJECT_RAIL_MIN_WIDTH, startWidth \+ delta\)\)/)
})

test('ProjectFilesRail — renders breadcrumb + sort group markers and the recursive tree', () => {
  // Breadcrumb chrome + sort toggle + recursive tree-node mount must all
  // remain wired so the rail keeps its directory-navigation shape.
  assert.match(source, /breadcrumbs\.map\(\(crumb, index\) =>/)
  assert.match(source, /onSortModeChange\(\(current\) => current === 'recent' \? 'name' : 'recent'\)/)
  assert.match(source, /<ProjectRailTreeNode/)
})
