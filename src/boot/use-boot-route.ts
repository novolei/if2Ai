// Phase M2.6 audit fix — minimal `useBootRoute()` hook that
// derives the 4-state boot route from boot-local state + the
// canonical activation projection.
//
// 4 states (mirrors the file-level plan §5.6):
//
//   - `show_splash`         → initial loading
//   - `show_onboarding`     → onboarding flow
//   - `show_activation_gate`→ onboarding complete but the activation
//                              snapshot blocks main shell entry
//   - `show_main_shell`     → fully ready
//
// Inputs:
//
//   - `showSplash` / `showOnboarding`  legacy boolean flags owned
//                                      by `App.tsx`. Keeping them
//                                      as inputs avoids touching
//                                      the boot-orchestration
//                                      `useEffect` in this slice.
//
//   - `snapshot.activation`            canonical projection from the
//                                      runtime-projection store.
//                                      `null` is treated as
//                                      "unknown / fetch in flight":
//                                      do NOT block main shell on
//                                      unknown — the overlay catches
//                                      revoke-while-running cases.
//
// Honest scope:
//
// - This hook does NOT itself fetch activation status. The
//   bridge does that on `wireRuntimeProjectionListeners`.
// - This hook does NOT migrate the splash / onboarding state
//   ownership. That stays in `App.tsx` until M3+ memory coordinator
//   (where the boot orchestrator becomes worth its own module).
// - The 4-state output is a derivation; the BootShell continues to
//   render `<ActivationGateOverlay/>` as the fallback for the
//   "revoke during active main session" case so the gate stays
//   reachable even in the `show_main_shell` state.

import { useRuntimeProjectionSelector } from '@/runtime-projection'
import type { BootSurface } from './BootShell'

export type BootRouteDecision =
  | 'show_splash'
  | 'show_onboarding'
  | 'show_activation_gate'
  | 'show_main_shell'

export interface UseBootRouteInput {
  /** `true` while the splash screen should be displayed. */
  showSplash: boolean
  /** `true` while the onboarding flow should be displayed. */
  showOnboarding: boolean
}

/**
 * Compute the canonical 4-state boot route from boot-local flags
 * and the activation projection.
 */
export function useBootRoute({
  showSplash,
  showOnboarding,
}: UseBootRouteInput): BootRouteDecision {
  const activation = useRuntimeProjectionSelector((s) => s.activation)
  if (showSplash) return 'show_splash'
  if (showOnboarding) return 'show_onboarding'
  if (activation && !activation.allowsMainShell) {
    return 'show_activation_gate'
  }
  return 'show_main_shell'
}

/**
 * Map the canonical 4-state route into the BootShell's
 * 3-state `surface` prop.  `show_activation_gate` is rendered as
 * the main surface so the existing `<ActivationGateOverlay/>`
 * (mounted inside BootShell) takes over the screen — the overlay
 * is the activation-gate UI today.
 */
export function bootRouteToSurface(route: BootRouteDecision): BootSurface {
  switch (route) {
    case 'show_splash':
      return 'splash'
    case 'show_onboarding':
      return 'onboarding'
    case 'show_activation_gate':
    case 'show_main_shell':
      return 'main'
  }
}
