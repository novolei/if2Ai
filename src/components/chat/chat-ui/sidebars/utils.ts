/**
 * GF-01 PR-07 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Shared helpers + sizing constants for the project-files rail sidebar.
 * Render-equivalent move: every helper body and constant value is
 * preserved verbatim from the original inline definition so the rail
 * layout, sort order and preview-language inference cannot drift.
 */

import type { DirectoryEntryPreview, FilePreviewPayload } from "@/lib/tauri"

/** Lower bound (px) for the project-files rail width drag handle. */
export const PROJECT_RAIL_MIN_WIDTH = 140
/** Upper bound (px) for the project-files rail width drag handle. */
export const PROJECT_RAIL_MAX_WIDTH = 300

/**
 * Map a preview file's name + payload kind to a syntax-highlight language
 * tag for the rail's `ReactMarkdown` code-fence rendering. Markdown is
 * recognised by extension or by the explicit payload kind; common code
 * extensions are passed through unchanged. Unknown extensions fall back
 * to `text` so the highlighter never throws.
 */
export function inferPreviewLanguage(fileName: string, kind?: FilePreviewPayload['kind']) {
  const ext = fileName.split('.').pop()?.toLowerCase() ?? ''
  if (kind === 'markdown' || ext === 'md' || ext === 'markdown') return 'markdown'
  if (ext === 'ts' || ext === 'tsx' || ext === 'js' || ext === 'jsx') return ext
  if (ext === 'rs' || ext === 'py' || ext === 'json' || ext === 'css' || ext === 'html' || ext === 'sh' || ext === 'md' || ext === 'sql' || ext === 'yaml' || ext === 'yml') return ext
  return 'text'
}

/**
 * Stable directory sort: folders always before files; within a kind,
 * either most-recently-modified first (`recent`) or natural-order name
 * compare (`name`). Returns a fresh array so callers can pass it to
 * `useState` setters without aliasing.
 */
export function sortRailDirectoryEntries(entries: DirectoryEntryPreview[], sortMode: 'recent' | 'name') {
  const next = [...entries]
  next.sort((a, b) => {
    if (a.kind !== b.kind) {
      return a.kind === 'folder' ? -1 : 1
    }
    if (sortMode === 'recent') {
      const aTime = a.modified_ms ?? 0
      const bTime = b.modified_ms ?? 0
      if (aTime !== bTime) return bTime - aTime
    }
    return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
  })
  return next
}
