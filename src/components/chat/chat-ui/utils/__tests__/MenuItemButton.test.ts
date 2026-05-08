import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../MenuItemButton.tsx', import.meta.url), 'utf8')

test('MenuItemButton — exports a named function and uses DropdownMenuItem', () => {
  assert.match(source, /export function MenuItemButton\(/)
  assert.match(source, /import \{ DropdownMenuItem \}/)
  assert.match(source, /onSelect=\{\(\) => onClick\?\.\(\)\}/)
})

test('MenuItemButton — preserves the active-vs-inactive class fork', () => {
  assert.match(source, /'h-8 rounded-\[6px\] px-2\.5 text-\[12\.5px\] font-medium'/)
  assert.match(source, /active \? 'bg-accent text-accent-foreground' : 'text-popover-foreground'/)
})
