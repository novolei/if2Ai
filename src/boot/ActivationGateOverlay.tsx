// Phase M2.6 — boot activation gate.
//
// Replaces the M2.5 placeholder with a full SwiftUI-port modal
// (`ActivationGate`) wired to the new `iclaw-activation-server`
// IPC surface.  Mount semantics are unchanged:
//
//   - `snapshot.activation === null`           → render nothing
//     (boot is still loading; let the splash stay in charge).
//   - `snapshot.activation.allowsMainShell`     → render nothing
//     (main shell is allowed; nothing to gate).
//   - otherwise                                 → render the gate.
//
// Honest scope: the gate runs the full SwiftUI ceremony — Fibonacci
// digit sphere, 8-cell OTP with five visual phases, automatic /
// manual code paths, beta-invite badge, device indicator.  The
// underlying lifecycle hits the real activation server via the
// `activation_request_license / poll / redeem(_by_code) / refresh
// / revoke_check / deactivate` IPC family.

import { useCallback } from 'react'

import {
  refreshActivationSnapshot,
  useRuntimeProjectionSelector,
} from '@/runtime-projection'

import { ActivationGate } from './activation/ActivationGate'

export function ActivationGateOverlay() {
  const activation = useRuntimeProjectionSelector((s) => s.activation)

  const handleClose = useCallback(() => {
    void refreshActivationSnapshot()
  }, [])

  if (!activation) return null
  if (activation.allowsMainShell) return null

  return <ActivationGate onCloseRequested={handleClose} />
}
