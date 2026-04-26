import { useEffect, useState } from 'react'

import { APP_VERSION_LABEL } from '@/lib/appVersion'
import { activationGetInstallationId } from '@/lib/tauri'

/**
 * Discreet version watermark anchored to the bottom-right of the main window.
 *
 * Layout:
 *   `<device-indicator> · v<APP_VERSION>`
 *
 * The device indicator is the same 8-char `XXXX-XXXX` shown on the activation
 * modal — fetched once via `activation_get_installation_id` (cheap, hashed).
 * If the IPC fails (e.g. boot race), we render the version-only fallback so
 * the watermark never disappears.
 */
export function AppVersionWatermark() {
  const [deviceIndicator, setDeviceIndicator] = useState('')

  useEffect(() => {
    let cancelled = false
    activationGetInstallationId()
      .then((id) => {
        if (!cancelled) setDeviceIndicator(id.deviceIndicator)
      })
      .catch((err) => {
        // eslint-disable-next-line no-console
        console.warn('[AppVersionWatermark] installation_id fetch failed', err)
      })
    return () => {
      cancelled = true
    }
  }, [])

  const label = deviceIndicator
    ? `${deviceIndicator} · ${APP_VERSION_LABEL}`
    : APP_VERSION_LABEL

  return (
    <div
      aria-hidden="true"
      className="pointer-events-none fixed bottom-1.5 right-2.5 z-50 select-none font-mono text-[9.5px] font-medium tracking-wide text-muted-foreground/70"
      title={`If2Ai ${APP_VERSION_LABEL}${deviceIndicator ? ` (device ${deviceIndicator})` : ''}`}
    >
      {label}
    </div>
  )
}
