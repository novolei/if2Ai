import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../SlashCommandSuggestions.tsx', import.meta.url), 'utf8')

test('SlashCommandSuggestions — exports a function component with typed props', () => {
  assert.match(source, /export function SlashCommandSuggestions\(/)
  assert.match(source, /export interface SlashCommandSuggestionsProps/)
})

test('SlashCommandSuggestions — surfaces the rawInput query in the header label', () => {
  assert.match(source, /rawInput\?: string/)
  assert.match(source, /\{rawInput \?\? '\/'\}/)
})
