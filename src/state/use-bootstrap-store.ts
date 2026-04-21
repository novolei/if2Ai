// MIG-013 — React adapter for the bootstrap store.
//
// Mirrors the hook shape already established by
// `src/runtime-projection/use-runtime-projection.ts`:
// `useSyncExternalStore` with an optional selector so consumers
// only re-render on the slice they care about.

import { useCallback, useSyncExternalStore } from 'react'

import { bootstrapStore, type BootstrapState, type BootstrapStore } from './bootstrap-store.ts'

/** Subscribe to the full bootstrap state snapshot. */
export function useBootstrapState(store: BootstrapStore = bootstrapStore): BootstrapState {
  const subscribe = useCallback((cb: () => void) => store.subscribe(cb), [store])
  const get = useCallback(() => store.getSnapshot(), [store])
  return useSyncExternalStore(subscribe, get, get)
}

/** Subscribe to a slice of the bootstrap state. */
export function useBootstrapSelector<T>(
  selector: (state: BootstrapState) => T,
  store: BootstrapStore = bootstrapStore,
): T {
  const subscribe = useCallback((cb: () => void) => store.subscribe(cb), [store])
  const get = useCallback(() => selector(store.getSnapshot()), [store, selector])
  return useSyncExternalStore(subscribe, get, get)
}
