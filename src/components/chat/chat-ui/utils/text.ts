/**
 * Small text formatting helpers shared by chat-ui leaf components.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) without
 * behavioural change. Each function preserves its original semantics so
 * callers can be migrated incrementally.
 */

/**
 * Format a Date as `HH:MM` in zh-CN, 24h. Returns empty string for
 * invalid Date inputs (matches the legacy guard).
 */
export function formatShortTime(date: Date): string {
  if (!(date instanceof Date) || Number.isNaN(date.getTime())) return ''

  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(date)
}

/**
 * Render a millisecond duration as either `<n>ms` (sub-second) or
 * `<n.n>s` (one decimal place).
 */
export function formatDuration(value: number): string {
  if (value < 1000) return `${value}ms`
  return `${(value / 1000).toFixed(1)}s`
}

/**
 * Truncate a string with a trailing ellipsis once it exceeds maxLen.
 */
export function truncateText(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text
  return text.slice(0, maxLen) + '…'
}

/**
 * Mask common credentials (token / password / secret / api[_-]?key /
 * Bearer …) inside an arbitrary text blob. Returns input unchanged
 * when empty.
 */
export function redactSensitiveText(text: string): string {
  if (!text) return text
  return text
    .replace(/(token|password|secret|api[_-]?key)\s*[:=]\s*[^\s]+/gi, '$1=<redacted>')
    .replace(/Bearer\s+[A-Za-z0-9._-]+/g, 'Bearer <redacted>')
}

/**
 * Produce the inline summary label shown for a collapsed thinking
 * block. Matches legacy chat-ui formatting word-for-word.
 */
export function summarizeThinkingText(thinking: string): string {
  const lineCount = thinking.split('\n').filter((line) => line.trim().length > 0).length
  if (lineCount <= 0) return '已完成思考'
  return lineCount === 1 ? '已完成上下文判断' : `已完成上下文判断 · ${lineCount} 段`
}
