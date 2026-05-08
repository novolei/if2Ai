/**
 * Tool-call display helpers — barrel re-export. Split into `builders.ts`
 * (pure data + status helpers) and `glyphs.tsx` (JSX components) to stay
 * under the 500-line per-file cap (GF-01 PR-02).
 */

export type { ChatToolStatus, ToolDisplay } from './builders'
export {
  buildToolCallDisplay,
  buildToolCommandResultLine,
  buildToolDetailLines,
  firstNonEmptyLine,
  isToolFailureStatus,
  isToolPendingStatus,
  normalizeToolLabel,
  normalizeToolStatus,
  pickToolHeadline,
  pickToolString,
  shortenMiddle,
  summarizeToolResult,
  tryParseJson,
} from './builders'

export {
  ToolStatusGlyph,
  getAssistantStatusMeta,
  getToolCallGlyph,
  renderInlineToolSummary,
} from './glyphs'
