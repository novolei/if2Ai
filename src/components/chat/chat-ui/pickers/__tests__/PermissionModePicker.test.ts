import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../PermissionModePicker.tsx', import.meta.url), 'utf8')
const itemsSource = readFileSync(
  new URL('../../utils/items.ts', import.meta.url),
  'utf8',
)

test('PermissionModePicker — exports a function component with typed props', () => {
  assert.match(source, /export function PermissionModePicker\(/)
  assert.match(source, /export interface PermissionModePickerProps/)
  assert.match(source, /permissionMode: PermissionMode/)
  assert.match(source, /setPermissionMode: React\.Dispatch<React\.SetStateAction<PermissionMode>>/)
})

test('PermissionModePicker — renders all permission mode option values from items.ts', () => {
  // Driven by permissionModeItems in utils/items.ts; ensure the canonical
  // mode values are still present so the dropdown surfaces every option.
  assert.match(itemsSource, /value: 'dangerFullAccess'/)
  assert.match(itemsSource, /value: 'workspaceWrite'/)
  assert.match(itemsSource, /value: 'readOnly'/)
  assert.match(source, /permissionModeItems\.map\(\(item\) => \(/)
  assert.match(source, /onClick=\{\(\) => setPermissionMode\(item\.value\)\}/)
})
