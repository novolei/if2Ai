// Runtime projection bridge (Phase M2.4).
//
// Wires the three real Tauri event sources into the runtime
// projection store via the `translator` family.  This is the only
// place in the frontend that turns a backend wire payload into a
// `CanonicalRuntimeEvent`; page components MUST NOT do this work
// themselves.
//
//   `agent-token`         -> translateAgentTokenPayload
//   `permission-request`  -> translatePermissionRequestPayload
//   `memory_event`        -> translateMemoryEventPayload
//
// Note: the bridge subscribes **broadly** to `agent-token` (no
// stream-id filter). Existing `listenToStream(streamId, ...)`
// callbacks in `App.tsx` / `ChatWorkspace` continue to drive the
// current chat UI — they run **in parallel** with the projection
// pipeline so this slice does not have to rewrite the chat
// rendering surface. M2.5+ will progressively swap UI consumers
// over to the projection store and the per-stream listeners can
// then retire.
//
// Phase M2.5 — activation snapshot now flows through this bridge
// via a **fetch seam**, not a Tauri event.  On `wireRuntimeProjectionListeners`
// invocation we call `activation_get_status` once and dispatch the
// translated `ActivationSnapshotEvent`.  The optional
// `refreshActivationSnapshot()` helper exposed below lets future
// boot-shell layers re-fetch on demand (e.g. after a settings
// refresh).  When the backend grows a real `activation_status_changed`
// Tauri event, this bridge can swap fetch → listen without touching
// the translator or reducer.
//
// Phase M2.6 — execution-mode decision now flows through the same
// bridge via a **fetch seam** as well: the
// `refreshExecutionModeDecision(input)` helper calls the new
// `request_intelligence_classify` IPC command and dispatches the
// translated `ExecutionModeDecisionEvent`.  Today's classifier is
// deterministic + heuristic + advisory-only — it is **not**
// auto-routed into the agent loop. The bridge does NOT auto-fetch
// on wire; consumers (m2.6 UI hooks) call the helper explicitly so
// the UX never pretends the agent has been re-routed.
//
// Out of scope for this slice:
// - License-refresh / revoke-check / deactivate IPC commands —
//   `LicenseLifecycleService` is still placeholder-only on the
//   backend; the snapshot returned today reflects legacy onboarding
//   completion, not a real remote license.
// - `execution_mode_decision_emitted` Tauri event source —
//   `commands/agent.rs` only `tracing::info!`s the decision today.
//   When that event family lands the bridge swaps fetch → listen.

import {
  activationGetStatus,
  listenActivationStatusChanged,
  listenMemoryAfterTurn,
  listenMemoryEvent,
  listenToAgentTokenStream,
  listenToPermissionRequests,
  requestIntelligenceClassify,
  type RequestIntelligenceClassifyInput,
} from '@/lib/tauri'
import { getSupervisorSnapshot } from '@/api/sessions'

import {
  runtimeProjectionStore,
  type RuntimeProjectionStore,
} from './runtime-projection-store'
import {
  translateActivationSnapshot,
  translateAgentTokenPayload,
  translateExecutionModeDecision,
  translateMemoryAfterTurn,
  translateMemoryEventPayload,
  translateMemoryWriteDecision,
  translatePermissionRequestPayload,
  translateSupervisorSnapshot,
} from './runtime-event-translator'

/** Cleanup handle returned by [`wireRuntimeProjectionListeners`]. */
export type RuntimeProjectionUnwire = () => void

/**
 * Subscribe the runtime projection store to the three real Tauri
 * event sources. Idempotent within one call site (multiple wirings
 * lead to multiple subscriptions); the caller is expected to invoke
 * the returned cleanup on unmount.
 *
 * Returns a synchronous disposer that detaches every successfully
 * registered listener. Async registration failures are logged
 * without throwing so a single bad listener never blocks the rest.
 */
export function wireRuntimeProjectionListeners(
  store: RuntimeProjectionStore = runtimeProjectionStore,
): RuntimeProjectionUnwire {
  const disposers: Array<() => void> = []
  let cancelled = false

  // Helper that handles "registered after caller already
  // unmounted" by immediately disposing the late-arriving handle.
  function track(
    promise: Promise<() => void>,
    label: string,
  ): void {
    promise
      .then((dispose) => {
        if (cancelled) {
          dispose()
          return
        }
        disposers.push(dispose)
      })
      .catch((err) => {
        // eslint-disable-next-line no-console
        console.error(
          `[runtime-projection-bridge] failed to register ${label} listener`,
          err,
        )
      })
  }

  track(
    listenToAgentTokenStream((payload) => {
      const event = translateAgentTokenPayload(payload)
      if (event) {
        store.dispatch(event)
      }
    }),
    'agent-token',
  )

  track(
    listenToPermissionRequests((payload) => {
      store.dispatch(translatePermissionRequestPayload(payload))
    }),
    'permission-request',
  )

  track(
    listenMemoryEvent((payload) => {
      store.dispatch(translateMemoryEventPayload(payload))
    }),
    'memory_event',
  )

  // Phase M3-C closeout — backend `memory_after_turn` batch
  // envelope.  Fires once per real turn end, even when the batch
  // is empty.  Bridge fan-out:
  //
  //   1. Always dispatch the batch canonical event
  //      (`memory_after_turn`) so `snapshot.memory.lastAfterTurn`
  //      is updated even for empty batches — closes the M3-B
  //      "empty batch is unobservable" audit gap.
  //   2. For non-empty batches, additionally dispatch one
  //      `memory_write_decision` canonical event per decision so
  //      the existing per-decision rolling ring
  //      (`snapshot.memory.writeDecisions`) keeps populating.
  track(
    listenMemoryAfterTurn((payload) => {
      // (1) batch envelope — fires for every turn end, including
      // empty batches.  The `quality` / `conflicts` arrays travel
      // with this event so M4 governance consumers can read
      // them without re-fetching.
      store.dispatch(translateMemoryAfterTurn(payload))

      // (2) per-decision fan-out for the rolling ring.  Empty
      // batches skip this loop naturally.  candidateId is
      // synthesised since the backend doesn't carry one today.
      for (const decision of payload.decisions) {
        const candidateId = `${payload.policyVersion}::${decision.decidedAt}`
        store.dispatch(translateMemoryWriteDecision(candidateId, decision))
      }
    }),
    'memory_after_turn',
  )

  // Phase M2.5 — one-shot activation snapshot fetch on wire.  The
  // command is cheap (reads local onboarding state) and the
  // translated event lands a typed `snapshot.activation` projection
  // before the boot shell decides what to render.  Failure leaves
  // `snapshot.activation` as `null`; consumers MUST treat `null` as
  // "unknown", not "blocked".
  void refreshActivationSnapshot(store)

  // T-007 — one-shot supervisor snapshot fetch on wire.  Reads the
  // session supervisor's current lifecycle state so the UI can
  // render active/blocked/recoverable labels without assembling
  // state from scattered sources.  Failure leaves `snapshot.supervisor`
  // as `null`; consumers MUST treat `null` as "unknown".
  void refreshSupervisorSnapshot(store)

  // Server-driven transitions: the Rust `lifecycle_manager` emits
  // `activation_status_changed` when its periodic `revoke_check`
  // observes a state delta (e.g. admin revoke on the VPS).  Refresh
  // the projection so `<ActivationGateOverlay/>` re-evaluates and
  // pops the modal without requiring an app restart.
  track(
    listenActivationStatusChanged(() => {
      void refreshActivationSnapshot(store)
    }),
    'activation_status_changed',
  )

  return () => {
    cancelled = true
    while (disposers.length > 0) {
      const fn = disposers.pop()
      if (fn) {
        try {
          fn()
        } catch (err) {
          // eslint-disable-next-line no-console
          console.error('[runtime-projection-bridge] disposer error', err)
        }
      }
    }
  }
}

/**
 * Re-fetch the canonical activation snapshot from the backend and
 * dispatch the translated event into the projection store.
 * Intended for boot-shell flows that want to refresh after the user
 * completes / resets onboarding without waiting for the next
 * `wireRuntimeProjectionListeners` call.
 *
 * Failure is logged and swallowed — `snapshot.activation` stays at
 * its last value (or `null` if no fetch ever succeeded).
 */
export async function refreshActivationSnapshot(
  store: RuntimeProjectionStore = runtimeProjectionStore,
): Promise<void> {
  try {
    const payload = await activationGetStatus()
    store.dispatch(translateActivationSnapshot(payload))
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(
      '[runtime-projection-bridge] activation_get_status failed',
      err,
    )
  }
}

/**
 * Phase M2.6 — run the deterministic classifier against `input` and
 * dispatch the translated [`ExecutionModeDecisionEvent`] into the
 * projection store.
 *
 * Honest framing: the dispatched decision is the classifier's
 * **judgment** of `input.userMessage`, not a routing action.  The
 * agent loop continues to run its existing single execution path
 * regardless. Consumers (chat-ui input chip, future preview hooks)
 * call this on demand with a draft message; failure is logged and
 * swallowed so the UX never blocks on classifier errors.
 *
 * Optional `runId` is forwarded into the event for forward-compat
 * with M2.7+ envelope wiring (today no caller has a `runId` for a
 * draft).
 */
export async function refreshExecutionModeDecision(
  input: RequestIntelligenceClassifyInput,
  options: { runId?: string; store?: RuntimeProjectionStore } = {},
): Promise<void> {
  const store = options.store ?? runtimeProjectionStore
  try {
    const payload = await requestIntelligenceClassify(input)
    store.dispatch(translateExecutionModeDecision(payload, options.runId))
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(
      '[runtime-projection-bridge] request_intelligence_classify failed',
      err,
    )
  }
}

/**
 * T-007 — one-shot supervisor snapshot fetch. Called on wire and on
 * demand when the active session changes. Failure leaves
 * `snapshot.supervisor` as `null`.
 */
export async function refreshSupervisorSnapshot(
  store: RuntimeProjectionStore = runtimeProjectionStore,
  sessionId?: string,
): Promise<void> {
  try {
    const payload = await getSupervisorSnapshot(sessionId ?? '')
    store.dispatch(translateSupervisorSnapshot(payload))
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(
      '[runtime-projection-bridge] get_supervisor_snapshot failed',
      err,
    )
  }
}
