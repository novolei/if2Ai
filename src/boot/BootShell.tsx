// Phase M2.7 — minimal boot shell.
//
// Single mount point for the three top-level surfaces App.tsx used
// to render inline:
//
//   - `surface === 'splash'`        → <If2AiLoadingScreen/>
//   - `surface === 'onboarding'`    → <OnboardingApp/>
//   - `surface === 'main'`          → render children (the
//                                      `<MainShell/>` tree).
//
// Plus two **always-mounted** boot-level overlays the rest of the
// app must never have to re-implement:
//
//   - `<Toaster/>`                  app-global notifications.
//   - `<ActivationGateOverlay/>`    canonical activation projection
//                                   surface; renders only when
//                                   `snapshot.activation` says the
//                                   main shell is currently blocked.
//
// Honest scope (mirrors the file-level plan §5.6 + the M2.7 task):
//   - This is a **container**, not a router.
//   - Boot decision logic stays in `App.tsx` (it owns the splash /
//     onboarding state today). M2.7 only consolidates the JSX
//     ownership; it does NOT replace the existing
//     `useEffect` boot orchestration.
//   - `ActivationGateOverlay` is a projection consumer, not a
//     mediator: it self-gates against `snapshot.activation`. The
//     boot shell does not pass any activation state to it.
//   - There is no specialized-surface routing. That lands later.

import type { ReactNode } from 'react'

import { ActivationGateOverlay } from './ActivationGateOverlay'
import { If2AiLoadingScreen } from '@/components/loading/If2AiLoadingScreen'
import { OnboardingApp } from '@/modules/onboarding/OnboardingApp'
import { Toaster } from 'sonner'

export type BootSurface = 'splash' | 'onboarding' | 'main'

export interface BootShellProps {
  /** Which top-level surface should render. */
  surface: BootSurface
  /** Forwarded to splash + onboarding for the macOS title-bar
   * drag handle. */
  onWindowDrag: (event: { clientX: number; clientY: number }) => void
  /** Onboarding completion callback (forwarded to `OnboardingApp`). */
  onOnboardingComplete: () => void
  /** Main-surface content. Rendered only when `surface === 'main'`. */
  children: ReactNode
}

export function BootShell({
  surface,
  onWindowDrag,
  onOnboardingComplete,
  children,
}: BootShellProps) {
  return (
    <>
      {surface === 'onboarding' ? (
        <OnboardingApp
          onWindowDrag={onWindowDrag}
          onComplete={onOnboardingComplete}
        />
      ) : surface === 'splash' ? (
        <If2AiLoadingScreen
          projectName="UClaw"
          stageLabel="Initializing agent workspace"
          onWindowDrag={onWindowDrag}
        />
      ) : (
        children
      )}
      {/* Always-on app-global surfaces.  Toaster needs to render
          notifications during boot too; ActivationGateOverlay must
          be able to overlay even the main shell when the canonical
          activation projection says the main shell is blocked. */}
      <Toaster position="bottom-right" richColors closeButton />
      <ActivationGateOverlay />
    </>
  )
}
