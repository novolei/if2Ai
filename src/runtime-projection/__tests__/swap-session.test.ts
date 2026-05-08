// ER-03 — runtimeProjectionStore.swapSession() atomic op +
// reducer rejection of mismatched-session events.
//
// See docs/IMPROVEMENTS-2026-05-05.md §3 ER-03.

import { strict as assert } from "node:assert";
import { describe, it } from "node:test";

import { createRuntimeProjectionStore } from "../runtime-projection-store.ts";
import { reduceRuntimeEventBatch } from "../runtime-event-reducer.ts";
import {
  emptyProjectionSnapshotForSession,
  type CanonicalRuntimeEvent,
} from "../types.ts";

describe("ER-03 swap-session atomic op", () => {
  it("(a) swapSession('s2') from 's1' clears snapshot and binds new currentSessionId", () => {
    const store = createRuntimeProjectionStore({ flushMode: "sync" });
    // Establish s1 with a bound run + some text.
    store.swapSession("s1");
    store.dispatch({
      kind: "stream_run_bound",
      runId: "r1",
      sessionId: "s1",
      receivedAt: 1,
    });
    store.dispatch({
      kind: "stream_text_delta",
      runId: "r1",
      text: "hello",
      receivedAt: 2,
    });
    store.flush();
    assert.equal(store.getSnapshot().currentSessionId, "s1");
    assert.ok(store.getSnapshot().runs["r1"]);

    // Swap to s2 — snapshot must be empty, guard must be 's2'.
    store.swapSession("s2");
    const after = store.getSnapshot();
    assert.equal(after.currentSessionId, "s2");
    assert.deepEqual(after.runs, {});
    assert.deepEqual(after.approvals, {});
    assert.equal(after.activation, null);
    assert.equal(after.executionMode, null);
  });

  it("(b) swapSession(null) clears snapshot and disables the guard", () => {
    const store = createRuntimeProjectionStore({ flushMode: "sync" });
    store.swapSession("s1");
    store.dispatch({
      kind: "stream_run_bound",
      runId: "r1",
      sessionId: "s1",
      receivedAt: 1,
    });
    store.flush();

    store.swapSession(null);
    const after = store.getSnapshot();
    assert.equal(after.currentSessionId, null);
    assert.deepEqual(after.runs, {});
  });

  it("(c) reducer drops events whose session id differs from currentSessionId", () => {
    const base = emptyProjectionSnapshotForSession("s2");
    const events: CanonicalRuntimeEvent[] = [
      // Direct sessionId mismatch — dropped.
      {
        kind: "stream_run_bound",
        runId: "r-old",
        sessionId: "s1",
        receivedAt: 10,
      },
      // Run-only delta for an unknown run while a guard is active —
      // dropped (its `stream_run_bound` was wiped during swap).
      {
        kind: "stream_text_delta",
        runId: "r-old",
        text: "leak",
        receivedAt: 11,
      },
      // Permission for the wrong session — dropped.
      {
        kind: "permission_request",
        sessionId: "s1",
        toolName: "x",
        permissionMode: "ask",
        currentMode: "ask",
        message: "m",
        receivedAt: 12,
      },
    ];
    const next = reduceRuntimeEventBatch(base, events);
    assert.deepEqual(next.runs, {});
    assert.deepEqual(next.approvals, {});
    assert.equal(next.currentSessionId, "s2");
  });

  it("(d) reducer accepts matching events, and accepts everything when currentSessionId is null", () => {
    // Matching session id under active guard.
    const guarded = emptyProjectionSnapshotForSession("s2");
    const accepted: CanonicalRuntimeEvent[] = [
      {
        kind: "stream_run_bound",
        runId: "r-new",
        sessionId: "s2",
        receivedAt: 20,
      },
      {
        kind: "stream_text_delta",
        runId: "r-new",
        text: "ok",
        receivedAt: 21,
      },
    ];
    const next = reduceRuntimeEventBatch(guarded, accepted);
    assert.equal(next.runs["r-new"]?.text, "ok");
    assert.equal(next.runs["r-new"]?.sessionId, "s2");

    // Null guard — legacy behavior accepts all.
    const unguarded = emptyProjectionSnapshotForSession(null);
    const mixed: CanonicalRuntimeEvent[] = [
      {
        kind: "stream_run_bound",
        runId: "r-a",
        sessionId: "sX",
        receivedAt: 30,
      },
      {
        kind: "stream_text_delta",
        runId: "r-a",
        text: "x",
        receivedAt: 31,
      },
    ];
    const next2 = reduceRuntimeEventBatch(unguarded, mixed);
    assert.equal(next2.runs["r-a"]?.text, "x");
  });
});
