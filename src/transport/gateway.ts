// MIG-010 — Local gateway bootstrap transport.
//
// Stable, narrow seam the frontend bootstrap path uses to discover
// "where do I talk to the backend?" and "is the backend ready to
// accept business calls?" without depending on any individual
// Tauri business command.
//
// This is the first frontend module that intentionally lives
// outside the historical `src/lib/tauri.ts` god-bridge: future
// `src/api/*` facades layer on top of these primitives, never on
// `invoke('run_agent_turn', ...)` style direct calls.
//
// Wire shape mirrors `src-tauri/src/modules/application/gateway_service.rs`.

import { invoke } from '@tauri-apps/api/core'

/** Schema version of the gateway bootstrap contract. Mirrored
 * from the backend `GATEWAY_SCHEMA_VERSION` constant. The
 * frontend bootstrap path SHOULD refuse to proceed if the
 * backend reports a different version. */
export const GATEWAY_SCHEMA_VERSION = '1.0.0'

/** Active transport tag. Today only `tauriIpc`; `localHttp` is
 * reserved for the post-MIG-011 sidecar bootstrap. */
export type GatewayTransport = 'tauriIpc' | 'localHttp'

/** High-level readiness state surfaced by
 * [`getGatewayHealth`]. */
export type GatewayStatus = 'initializing' | 'ready' | 'degraded'

/** Wire payload returned by the `get_gateway_url` IPC command. */
export interface GatewayUrlPayload {
  /** Canonical entry URL. Synthetic today; real once the future
   * sidecar bootstrap lands. */
  url: string
  /** Schema version of this payload + GatewayHealthPayload. */
  schemaVersion: string
  /** Active transport tag. */
  transport: GatewayTransport
}

/** Wire payload returned by the `get_gateway_health` IPC command. */
export interface GatewayHealthPayload {
  /** High-level readiness state. */
  status: GatewayStatus
  /** Schema version of this payload. */
  schemaVersion: string
  /** Human-readable reason when `status === 'degraded'`. */
  reason?: string
  /** RFC3339 UTC timestamp when this health snapshot was taken. */
  checkedAt: string
}

/** Fetch the canonical gateway URL + transport tag. Pure /
 * synchronous on the backend; safe to call before any other
 * command. */
export async function getGatewayUrl(): Promise<GatewayUrlPayload> {
  return invoke<GatewayUrlPayload>('get_gateway_url')
}

/** Fetch a fresh readiness snapshot. Safe to poll. */
export async function getGatewayHealth(): Promise<GatewayHealthPayload> {
  return invoke<GatewayHealthPayload>('get_gateway_health')
}

/** Poll `getGatewayHealth` until the backend reports a terminal
 * status. Resolves with a `Ready` **or** `Degraded` snapshot
 * (degraded is non-fatal so the splash UI can surface the
 * `reason`); rejects only on (a) the timeout elapsing while the
 * status is still `Initializing`, or (b) a schema version
 * mismatch between frontend and backend.
 *
 * Frontend bootstrap usage:
 *
 * ```ts
 * await awaitGatewayReady({ timeoutMs: 10_000, intervalMs: 250 })
 * // safe to call business commands now
 * ```
 */
export async function awaitGatewayReady(opts?: {
  timeoutMs?: number
  intervalMs?: number
}): Promise<GatewayHealthPayload> {
  const timeoutMs = opts?.timeoutMs ?? 10_000
  const intervalMs = opts?.intervalMs ?? 250
  const deadline = Date.now() + timeoutMs

  let last: GatewayHealthPayload | undefined
  while (Date.now() < deadline) {
    const snapshot = await getGatewayHealth()
    last = snapshot

    if (snapshot.schemaVersion !== GATEWAY_SCHEMA_VERSION) {
      throw new Error(
        `gateway schema version mismatch: frontend=${GATEWAY_SCHEMA_VERSION} backend=${snapshot.schemaVersion}`,
      )
    }

    if (snapshot.status === 'ready') {
      return snapshot
    }
    if (snapshot.status === 'degraded') {
      // Degraded is non-fatal — return so the splash UI can
      // surface the reason and still allow opt-in business calls.
      return snapshot
    }
    await new Promise((r) => setTimeout(r, intervalMs))
  }

  throw new Error(
    `gateway readiness timeout after ${timeoutMs}ms (last status: ${last?.status ?? 'unknown'})`,
  )
}
