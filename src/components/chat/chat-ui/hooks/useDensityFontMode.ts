/**
 * GF-01 PR-08 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Persistence of the chat transcript's density / font preferences. The
 * effects mirror the original inline `useEffect` blocks one-for-one and
 * keep the same `localStorage` keys, so existing user preferences carry
 * over without migration.
 */

import * as React from 'react'

/** Two visual density modes the chat transcript supports. */
export type DensityMode = 'comfortable' | 'compact'
/** Two font-family modes the chat transcript supports. */
export type FontMode = 'sans' | 'serif'

/** localStorage key holding the persisted density mode. Bumped to
 * `V2` historically; preserved verbatim from chat-ui.tsx. */
export const CHAT_DENSITY_MODE_STORAGE_KEY = 'chatDensityModeV2'
/** localStorage key holding the persisted font mode. */
export const CHAT_FONT_MODE_STORAGE_KEY = 'chatFontModeV2'

/** Persist `density` / `font` to `localStorage` whenever they change.
 *
 * Today the chat-ui shell still receives both as props (the parent
 * `App.tsx` owns the canonical state), so this hook only owns the
 * write-through effect. Returning the values verbatim keeps the call
 * site mechanical: `const { densityMode, fontMode } = useDensityFontMode({ density, font })`. */
export function useDensityFontMode(input: {
  density: DensityMode
  font: FontMode
}): { densityMode: DensityMode; fontMode: FontMode } {
  const { density, font } = input

  React.useEffect(() => {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(CHAT_DENSITY_MODE_STORAGE_KEY, density)
  }, [density])

  React.useEffect(() => {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(CHAT_FONT_MODE_STORAGE_KEY, font)
  }, [font])

  return { densityMode: density, fontMode: font }
}
