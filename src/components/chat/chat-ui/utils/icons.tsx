/**
 * Inline SVG glyphs for chat-ui's font / density toggles.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01). Markup
 * is identical to the legacy declarations; downstream consumers only
 * pass an optional `className`.
 */

import * as React from 'react'

/** "Aa" rendered in the system sans-serif stack. */
export function FontSansIcon({ className }: { className?: string }): React.JSX.Element {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className} aria-hidden="true">
      <text
        x="2.3"
        y="11.2"
        fontSize="8.4"
        fontWeight="600"
        fontFamily="system-ui, -apple-system, Segoe UI, Roboto, sans-serif"
        fill="currentColor"
      >
        Aa
      </text>
    </svg>
  )
}

/** "Aa" rendered in a serif stack (mirror of FontSansIcon). */
export function FontSerifIcon({ className }: { className?: string }): React.JSX.Element {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className} aria-hidden="true">
      <text
        x="2.1"
        y="11.2"
        fontSize="8.4"
        fontWeight="600"
        fontFamily="ui-serif, Georgia, Cambria, Times New Roman, serif"
        fill="currentColor"
      >
        Aa
      </text>
    </svg>
  )
}

/** Three-line glyph used for compact density. */
export function DensityCompactIcon({ className }: { className?: string }): React.JSX.Element {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className} aria-hidden="true">
      <path d="M3 5h10M3 8h10M3 11h10" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  )
}

/** Three-line glyph used for comfortable density (wider y spacing). */
export function DensityComfortableIcon({ className }: { className?: string }): React.JSX.Element {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className} aria-hidden="true">
      <path d="M3 4h10M3 8h10M3 12h10" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  )
}
