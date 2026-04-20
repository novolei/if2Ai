// Runtime event queue (Phase M2.3).
//
// Buffers `CanonicalRuntimeEvent`s produced by the translator and
// flushes them in micro-batches to subscribers.  This is the single
// place where event ordering + batching policy lives, so the
// reducer / future stores never have to deal with React render
// timing.
//
// Design notes:
// - FIFO: events are flushed in the order they were pushed.
// - Micro-task flush: a flush is scheduled via `queueMicrotask` so
//   bursts of `text_delta` events from the same backend tick get
//   batched together without introducing a frame of latency.
// - Synchronous fallback: callers can opt into immediate flushing
//   via [`createRuntimeEventQueue({ flushMode: 'sync' })`] for tests
//   and dev tools.
// - No type assumptions about consumers: the queue only knows about
//   `CanonicalRuntimeEvent` from `./types` — reducer / store wiring
//   lives in callers.

import type { CanonicalRuntimeEvent } from './types'

export type RuntimeEventBatchHandler = (
  batch: CanonicalRuntimeEvent[],
) => void

export interface RuntimeEventQueue {
  /** Push one event onto the queue. Triggers a flush on the next
   * micro-task (or immediately when `flushMode === 'sync'`). */
  push(event: CanonicalRuntimeEvent): void
  /** Subscribe to flushed batches. Returns an unsubscribe function. */
  subscribe(handler: RuntimeEventBatchHandler): () => void
  /** Force an immediate synchronous flush of any buffered events.
   * Useful for tests and dev tools. */
  flush(): void
  /** Remove every buffered event without notifying subscribers.
   * Useful for hot-reload scenarios. */
  reset(): void
  /** Snapshot of buffered (not-yet-flushed) events. Read-only. */
  pending(): readonly CanonicalRuntimeEvent[]
}

export interface RuntimeEventQueueOptions {
  /** `'microtask'` (default) batches a tick of pushes; `'sync'`
   * flushes per push and is intended for tests / dev tools. */
  flushMode?: 'microtask' | 'sync'
}

/**
 * Construct a fresh queue. M2.4 stores wire one of these as a
 * module-level singleton; M2.2 / M2.3 only define the type so
 * unit tests and the dev inspector can each own their own.
 */
export function createRuntimeEventQueue(
  options: RuntimeEventQueueOptions = {},
): RuntimeEventQueue {
  const flushMode = options.flushMode ?? 'microtask'

  let buffer: CanonicalRuntimeEvent[] = []
  let flushScheduled = false
  const subscribers = new Set<RuntimeEventBatchHandler>()

  function notifyAll(batch: CanonicalRuntimeEvent[]): void {
    // Snapshot subscribers so a handler that unsubscribes during
    // notification does not skip a sibling.
    const snapshot = Array.from(subscribers)
    for (const handler of snapshot) {
      try {
        handler(batch)
      } catch (err) {
        // Surface the error to the console without blocking the
        // remaining subscribers.  Future M2.4 dev inspector can
        // intercept this.
        // eslint-disable-next-line no-console
        console.error('[runtime-event-queue] subscriber error', err)
      }
    }
  }

  function doFlush(): void {
    if (buffer.length === 0) {
      flushScheduled = false
      return
    }
    const batch = buffer
    buffer = []
    flushScheduled = false
    notifyAll(batch)
  }

  function scheduleFlush(): void {
    if (flushScheduled) return
    flushScheduled = true
    queueMicrotask(doFlush)
  }

  return {
    push(event) {
      buffer.push(event)
      if (flushMode === 'sync') {
        doFlush()
      } else {
        scheduleFlush()
      }
    },
    subscribe(handler) {
      subscribers.add(handler)
      return () => {
        subscribers.delete(handler)
      }
    },
    flush() {
      doFlush()
    },
    reset() {
      buffer = []
      flushScheduled = false
    },
    pending() {
      return buffer
    },
  }
}
