/**
 * Static label lookups used by the chat composer's pickers.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) verbatim;
 * data values and fallback labels match the legacy declarations.
 */

import type { PermissionMode } from '@/lib/tauri'

/** Built-in model dropdown items shown when no provider list is loaded. */
export const modelItems = [
  { value: 'gpt-5.4-mini', label: 'GPT-5.4-Mini' },
  { value: 'gpt-5.4', label: 'GPT-5.4' },
  { value: 'gpt-4.1', label: 'GPT-4.1' },
]

/** Reasoning strength dropdown items (低 / 中 / 高). */
export const strengthItems = [
  { value: 'low', label: '低' },
  { value: 'mid', label: '中' },
  { value: 'high', label: '高' },
]

/** Permission mode dropdown items (full access / restricted / read-only). */
export const permissionModeItems: Array<{ value: PermissionMode; label: string }> = [
  { value: 'dangerFullAccess', label: '完全访问权限' },
  { value: 'workspaceWrite', label: '受限访问' },
  { value: 'readOnly', label: '只读' },
]

/**
 * Resolve the human-readable label for a model value, with the legacy
 * fallback (`GPT-5.4-Mini`).
 */
export function modelLabelFor(value: string): string {
  return modelItems.find((item) => item.value === value)?.label ?? 'GPT-5.4-Mini'
}

/**
 * Resolve the human-readable label for a strength value, with the
 * legacy fallback (`中`).
 */
export function strengthLabelFor(value: string): string {
  return strengthItems.find((item) => item.value === value)?.label ?? '中'
}

/**
 * Resolve the human-readable label for a permission mode value, with
 * the legacy fallback (`完全访问权限`).
 */
export function permissionModeLabelFor(value: PermissionMode): string {
  return permissionModeItems.find((item) => item.value === value)?.label ?? '完全访问权限'
}
