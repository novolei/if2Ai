// Runtime projection store (Phase M2.4).
//
// Owns one [`RuntimeProjectionSnapshot`] for the whole frontend.
// Internally:
//
//   `dispatch(event)` -> queue.push -> queue subscriber fires
//                                      -> `reduceRuntimeEventBatch`
//                                      -> snapshot replaced
//                                      -> notify external-store subscribers
//
// Designed for `useSyncExternalStore` (the React 18+ idiomatic way
// to subscribe to a non-React data source). Any other consumer can
// equally call `getSnapshot()` / `subscribe()` directly — there is
// no React dependency in this file.
//
// Hard rules (mirror file-level plan §5.6 + §5.7):
// 1. The store does NOT do any normalization on its own — only the
//    translator family in `./runtime-event-translator` may.
// 2. Snapshots are immutable: every mutation produces a new object
//    via the reducer.
// 3. The store is process-global by design (one runtime, one truth).
//    Tests use [`createRuntimeProjectionStore`] to get a fresh
//    instance.

import type { CanonicalRuntimeEvent, RuntimeProjectionSnapshot } from './types.ts'
import { emptyProjectionSnapshot } from './types.ts'
import { createRuntimeEventQueue, type RuntimeEventQueue } from './runtime-event-queue.ts'
import { reduceRuntimeEventBatch } from './runtime-event-reducer.ts'

export type RuntimeProjectionListener = () => void

export interface RuntimeProjectionStore {
  /** Snapshot accessor. Stable identity per snapshot — re-renders
   * driven by `useSyncExternalStore` only fire when the snapshot
   * reference changes. */
  getSnapshot(): RuntimeProjectionSnapshot
  /** Subscribe to snapshot changes. Returns an unsubscribe fn. */
  subscribe(listener: RuntimeProjectionListener): () => void
  /** Push one canonical event into the underlying queue. The queue
   * batches via micro-task by default, so callers may dispatch in
   * tight loops without thrashing the reducer. */
  dispatch(event: CanonicalRuntimeEvent): void
  /** Borrow the underlying queue for advanced wiring (e.g. dev
   * inspector hooking into the same flush). The bridge module uses
   * this to feed translated events without round-tripping through
   * `dispatch`. */
  queue(): RuntimeEventQueue
  /** Force a synchronous flush of the underlying queue. Useful in
   * tests so reducer side-effects are observable before the next
   * micro-task tick. */
  flush(): void
  /** Reset to an empty snapshot. Useful for hot-reload + tests. */
  reset(): void
  /** T-020 — restore a serialized checkpoint snapshot. Replaces the
   * internal state with `snapshot`, firing listeners. */
  restoreSnapshot(snapshot: RuntimeProjectionSnapshot): void
}

export interface CreateRuntimeProjectionStoreOptions {
  /** Pass `'sync'` to bypass the micro-task batch — useful in
   * tests. Defaults to `'microtask'` (the queue default). */
  flushMode?: 'microtask' | 'sync'
}

/**
 * Construct a fresh, isolated store. The `runtimeProjectionStore`
 * singleton below uses this; tests / dev tools can build their own.
 */
export function createRuntimeProjectionStore(
  options: CreateRuntimeProjectionStoreOptions = {},
): RuntimeProjectionStore {
  let snapshot: RuntimeProjectionSnapshot = emptyProjectionSnapshot()
  const listeners = new Set<RuntimeProjectionListener>()

  const queue = createRuntimeEventQueue({ flushMode: options.flushMode })

  function notifyAll(): void {
    // Snapshot listeners so a listener that unsubscribes during
    // notification does not skip a sibling.
    const snap = Array.from(listeners)
    for (const fn of snap) {
      try {
        fn()
      } catch (err) {
        // eslint-disable-next-line no-console
        console.error('[runtime-projection-store] listener error', err)
      }
    }
  }

  // Wire the queue to the reducer once at construction time.
  queue.subscribe((batch) => {
    if (batch.length === 0) return
    const next = reduceRuntimeEventBatch(snapshot, batch)
    if (next === snapshot) return
    snapshot = next
    notifyAll()
  })

  return {
    getSnapshot() {
      return snapshot
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    dispatch(event) {
      queue.push(event)
    },
    queue() {
      return queue
    },
    flush() {
      queue.flush()
    },
    reset() {
      queue.reset()
      const fresh = emptyProjectionSnapshot()
      if (fresh !== snapshot) {
        snapshot = fresh
        notifyAll()
      }
    },
    restoreSnapshot(next: RuntimeProjectionSnapshot) {
      queue.reset()
      if (next !== snapshot) {
        snapshot = next
        notifyAll()
      }
    },
  }
}

/**
 * Process-global runtime projection store. Bridge wiring in
 * `./runtime-projection-bridge` feeds it; React consumers read it
 * via [`useRuntimeProjection`] / [`useRuntimeProjectionSelector`].
 */
export const runtimeProjectionStore: RuntimeProjectionStore =
  createRuntimeProjectionStore()
