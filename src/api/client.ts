// MIG-012 — frontend API facade transport abstraction.
//
// Narrow, swappable seam over Tauri `invoke` / `listen` so every
// `src/api/*` domain module talks to a single
// [`ApiClient`] rather than importing `@tauri-apps/api/core`
// directly. This is the single extension point the future
// sidecar / HTTP transport (post-MIG-011 / -015) will hook into;
// domain modules never learn which transport is active.
//
// Tests inject a mock client via [`setApiClient`] so every
// `src/api/*.ts` module becomes unit-testable without needing a
// Tauri window.

import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event'

/** Subset of `@tauri-apps/api/event`'s `Event<T>` the facade
 * surfaces to subscribers. Mirrors the fields the historical
 * `src/lib/tauri.ts` callbacks already destructure. */
export interface ApiEvent<T> {
  payload: T
}

/** Transport abstraction every `src/api/*` module depends on.
 *
 * - `call`: RPC-style request/response (Tauri `invoke` today).
 * - `subscribe`: fan-out event stream (Tauri `listen` today).
 */
export interface ApiClient {
  call<T>(command: string, args?: Record<string, unknown>): Promise<T>
  subscribe<T>(event: string, handler: (event: ApiEvent<T>) => void): Promise<UnlistenFn>
}

/** Default production client wired to the actual Tauri IPC. */
const defaultClient: ApiClient = {
  call<T>(command, args) {
    return tauriInvoke<T>(command, args)
  },
  async subscribe<T>(event, handler) {
    return await tauriListen<T>(event, (ev) => {
      handler({ payload: ev.payload })
    })
  },
}

let currentClient: ApiClient = defaultClient

/** Return the client every `src/api/*` module dispatches through.
 * Always returns the default Tauri client unless a test has
 * installed a mock via [`setApiClient`]. */
export function getApiClient(): ApiClient {
  return currentClient
}

/** Install a custom [`ApiClient`] implementation. Intended for
 * unit tests and future transport cutovers. Passing `null`
 * restores the default Tauri client. */
export function setApiClient(client: ApiClient | null): void {
  currentClient = client ?? defaultClient
}

/** Reset the module-level client to the default Tauri-backed
 * implementation. Shorthand for `setApiClient(null)` intended
 * for test `afterEach` hooks. */
export function resetApiClient(): void {
  currentClient = defaultClient
}
