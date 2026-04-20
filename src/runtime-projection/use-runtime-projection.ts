// React adapter for the runtime projection store (Phase M2.4).
//
// Two hooks:
//   * [`useRuntimeProjection`] — full snapshot subscription. Use
//     sparingly: rerenders on any change.
//   * [`useRuntimeProjectionSelector`] — selector subscription. The
//     hook only re-renders when the selector returns a new value
//     (`Object.is` comparison, matching `useSyncExternalStore`'s
//     default semantics).
//
// Hard rules:
// 1. No effects, no state, no heuristics — these are pure
//    `useSyncExternalStore` adapters.
// 2. Selectors run on every store notification; keep them cheap or
//    memoise upstream.

import { useCallback, useSyncExternalStore } from 'react'

import {
  runtimeProjectionStore,
  type RuntimeProjectionStore,
} from './runtime-projection-store'
import type { RuntimeProjectionSnapshot } from './types'

/** Subscribe the calling component to the entire runtime projection
 * snapshot. Returns the latest [`RuntimeProjectionSnapshot`]. */
export function useRuntimeProjection(
  store: RuntimeProjectionStore = runtimeProjectionStore,
): RuntimeProjectionSnapshot {
  const subscribe = useCallback(
    (cb: () => void) => store.subscribe(cb),
    [store],
  )
  const get = useCallback(() => store.getSnapshot(), [store])
  return useSyncExternalStore(subscribe, get, get)
}

/** Subscribe to a slice of the snapshot. The component only re-
 * renders when the selector returns a different value. */
export function useRuntimeProjectionSelector<T>(
  selector: (snap: RuntimeProjectionSnapshot) => T,
  store: RuntimeProjectionStore = runtimeProjectionStore,
): T {
  const subscribe = useCallback(
    (cb: () => void) => store.subscribe(cb),
    [store],
  )
  const get = useCallback(() => selector(store.getSnapshot()), [store, selector])
  return useSyncExternalStore(subscribe, get, get)
}
