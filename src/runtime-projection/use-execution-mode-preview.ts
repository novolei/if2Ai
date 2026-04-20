// Phase M2.6 — opt-in hook that calls the deterministic classifier
// for a draft message (debounced) and dispatches the decision into
// the projection store.
//
// Honest framing:
// - The hook only runs the classifier; it does **not** route the
//   agent. The chat input continues to send via the existing
//   `start_agent_stream` path.
// - The hook intentionally produces no return value beyond a no-op
//   cleanup. Consumers read the projection state via
//   `useRuntimeProjection*` hooks (typically the
//   `ExecutionModePill` component).
// - When `draft` is empty / shorter than `minLength`, the hook
//   does not call the classifier and does not clear an existing
//   projection (last decision lingers until the next non-empty
//   draft).

import { useEffect, useRef } from 'react'

import { refreshExecutionModeDecision } from './runtime-projection-bridge'
import {
  runtimeProjectionStore,
  type RuntimeProjectionStore,
} from './runtime-projection-store'

export interface UseExecutionModePreviewOptions {
  /** Debounce in milliseconds. Defaults to 300ms. */
  debounceMs?: number
  /** Minimum draft length before classifying. Defaults to 4 chars. */
  minLength?: number
  /** Optional session id forwarded into classifier evidence. */
  sessionId?: string
  /** Optional project id forwarded into classifier evidence. */
  projectId?: string
  /** Optional working directory forwarded into classifier evidence. */
  workdir?: string
  /** Override the target store (tests). Defaults to the singleton. */
  store?: RuntimeProjectionStore
}

/**
 * Watch `draft` and run the classifier whenever it changes.
 *
 * Returns nothing — consumers read the resulting projection via
 * `useRuntimeProjectionSelector(s => s.executionMode)` (typically
 * inside the `ExecutionModePill` component).
 */
export function useExecutionModePreview(
  draft: string,
  options: UseExecutionModePreviewOptions = {},
): void {
  const {
    debounceMs = 300,
    minLength = 4,
    sessionId,
    projectId,
    workdir,
    store = runtimeProjectionStore,
  } = options

  // Hold the latest options in a ref so the debounced callback uses
  // current values without forcing a re-subscription on every change.
  const latest = useRef({ sessionId, projectId, workdir, store })
  latest.current = { sessionId, projectId, workdir, store }

  useEffect(() => {
    const trimmed = draft.trim()
    if (trimmed.length < minLength) {
      return
    }
    const handle = window.setTimeout(() => {
      void refreshExecutionModeDecision(
        {
          userMessage: trimmed,
          sessionId: latest.current.sessionId,
          projectId: latest.current.projectId,
          workdir: latest.current.workdir,
        },
        { store: latest.current.store },
      )
    }, debounceMs)
    return () => {
      window.clearTimeout(handle)
    }
  }, [draft, debounceMs, minLength])
}
