// FEAT-INT-001 — Per-family ring buffer store for typed Agent
// Evolution events.
//
// Mirrors the manual `useSyncExternalStore`-compatible shape
// established by `bootstrap-store.ts` — no third-party dependency
// (zustand etc.) is added by this Pack.
//
// Wiring contract: the gateway listener calls
// `evolutionEventStore.applyEnvelope(rawEnvelope)` for every
// inbound envelope. The store delegates to the pure
// translator + reducer; UI selectors read the per-family arrays
// via `useEvolutionEventState` / `useEvolutionEventSelector`.
//
// No React component is provided by this Pack — only the store
// skeleton + a React adapter hook for downstream consumers.

import { useCallback, useSyncExternalStore } from 'react'

import {
  evolutionEventReducer,
  INITIAL_EVOLUTION_EVENT_STATE,
  type EvolutionEventState,
} from '../transport/runtime-event-reducer.ts'
import { translateEnvelope } from '../transport/runtime-event-translator.ts'

export type EvolutionEventListener = () => void

export interface EvolutionEventStore {
  getSnapshot(): EvolutionEventState
  subscribe(listener: EvolutionEventListener): () => void
  /** Apply one raw envelope from the gateway. Malformed / non-evolution
   * envelopes are silently dropped (translator returns `null`). */
  applyEnvelope(envelope: unknown): void
  /** Reset every family slice to empty — useful for hot reload + tests. */
  reset(): void
}

/** Create a fresh, isolated store instance. Tests call this
 * directly; production code uses the module-level
 * [`evolutionEventStore`] singleton below. */
export function createEvolutionEventStore(): EvolutionEventStore {
  let state: EvolutionEventState = INITIAL_EVOLUTION_EVENT_STATE
  const listeners = new Set<EvolutionEventListener>()

  const notify = () => {
    listeners.forEach((l) => l())
  }

  return {
    getSnapshot: () => state,
    subscribe: (listener) => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    applyEnvelope: (envelope) => {
      const event = translateEnvelope(envelope)
      if (event === null) return
      const next = evolutionEventReducer(state, event)
      if (next === state) return
      state = next
      notify()
    },
    reset: () => {
      if (state === INITIAL_EVOLUTION_EVENT_STATE) return
      state = INITIAL_EVOLUTION_EVENT_STATE
      notify()
    },
  }
}

/** Module-level singleton used by the production gateway listener
 * + React adapters. Tests should prefer `createEvolutionEventStore()`
 * for isolation. */
export const evolutionEventStore: EvolutionEventStore = createEvolutionEventStore()

// ---------------------------------------------------------------------------
// React adapters (mirrors `use-bootstrap-store.ts`)
// ---------------------------------------------------------------------------

/** Subscribe to the full evolution event snapshot. */
export function useEvolutionEventState(
  store: EvolutionEventStore = evolutionEventStore,
): EvolutionEventState {
  const subscribe = useCallback((cb: () => void) => store.subscribe(cb), [store])
  const get = useCallback(() => store.getSnapshot(), [store])
  return useSyncExternalStore(subscribe, get, get)
}

/** Subscribe to a slice of the evolution event state. */
export function useEvolutionEventSelector<T>(
  selector: (state: EvolutionEventState) => T,
  store: EvolutionEventStore = evolutionEventStore,
): T {
  const subscribe = useCallback((cb: () => void) => store.subscribe(cb), [store])
  const get = useCallback(() => selector(store.getSnapshot()), [store, selector])
  return useSyncExternalStore(subscribe, get, get)
}
