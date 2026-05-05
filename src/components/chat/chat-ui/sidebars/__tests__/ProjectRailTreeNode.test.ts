import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ProjectRailTreeNode.tsx', import.meta.url), 'utf8')

test('ProjectRailTreeNode — exports the memoised recursive node declaration', () => {
  // Memo wrapper + named factory must both survive the move so tree
  // sub-renders stay cheap and React DevTools labels are stable.
  assert.match(source, /export const ProjectRailTreeNode = React\.memo\(function ProjectRailTreeNode\(/)
})

test('ProjectRailTreeNode — recursively renders children of expanded folders', () => {
  // Recursion is the whole point of the tree node; losing it would flat-
  // ten subdirectories. The expanded branch must keep mounting itself.
  assert.match(source, /isFolder && isExpanded \?/)
  assert.match(source, /children\.map\(\(child\) => \(\s*<ProjectRailTreeNode/)
})
