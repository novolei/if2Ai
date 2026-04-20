import { APP_VERSION_LABEL } from '@/lib/appVersion'

/**
 * Discreet version watermark anchored to the top-left of the main window.
 *
 * Positioning:
 * - `fixed` so it overlays everything (sidebar + main pane).
 * - `left-[88px]` keeps clear of macOS traffic-light controls (which sit at
 *   ~12px–80px on `titleBarStyle: "Overlay"` windows).
 * - `top-1.5` aligns with the navbar baseline.
 * - `pointer-events-none` so it never intercepts drag/click.
 *
 * Single source of truth for the version: `package.json` → injected at build
 * time via Vite `define`. See `src/lib/appVersion.ts`.
 */
export function AppVersionWatermark() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none fixed left-[88px] top-1.5 z-50 select-none font-mono text-[9.5px] font-medium tracking-wide text-black/20 mix-blend-multiply"
      title={`If2Ai ${APP_VERSION_LABEL}`}
    >
      {APP_VERSION_LABEL}
    </div>
  )
}
