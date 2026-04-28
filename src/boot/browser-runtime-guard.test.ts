import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const mainSource = readFileSync(new URL('../main.tsx', import.meta.url), 'utf8')

test('browser runtime guard explains localhost vite without tauri bridge', () => {
  assert.match(mainSource, /function hasTauriRuntime\(\): boolean/)
  assert.match(mainSource, /'__TAURI_INTERNALS__' in window/)
  assert.match(mainSource, /function BrowserRuntimeDiagnostic\(\)/)
  assert.match(mainSource, /This URL is the Vite dev server/)
  assert.match(mainSource, /If2Ai needs the Tauri runtime bridge/)
  assert.match(mainSource, /if \(!hasTauriRuntime\(\)\)/)
})
