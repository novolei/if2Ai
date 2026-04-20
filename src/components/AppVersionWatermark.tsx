import { APP_VERSION_LABEL } from '@/lib/appVersion'

/**
 * Discreet version watermark anchored to the bottom-right of the main window.
 *
 * Positioning:
 * - `fixed` + `bottom-1.5 right-2.5` so it floats above any scrollable
 *   content without joining the layout flow.
 * - `pointer-events-none` so it never intercepts drag/click.
 * - `mix-blend-multiply` keeps it subtle on light backgrounds.
 *
 * Single source of truth for the version: `package.json` → injected at build
 * time via Vite `define`. See `src/lib/appVersion.ts`.
 */
export function AppVersionWatermark() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none fixed bottom-1.5 right-2.5 z-50 select-none font-mono text-[9.5px] font-medium tracking-wide text-black/25 mix-blend-multiply"
      title={`If2Ai ${APP_VERSION_LABEL}`}
    >
      {APP_VERSION_LABEL}
    </div>
  )
}
