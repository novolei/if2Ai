// Contract drift guardrails (T-018 / GAP-008).
//
// Verifies that every Rust `RuntimeEventType` + `payload_family`
// pair emitted by the backend has a corresponding translator
// function in the frontend.  Unmapped events fail fast so new
// backend event types don't silently land without projection
// coverage.
//
// The mapping table is the **manual source of truth** for the
// contract boundary.  When a new Rust event type or payload family
// is added, the corresponding row MUST be added here, or the test
// MUST be updated to document why it is intentionally unmapped.
//
// Backend reference:
//   src-tauri/src/modules/runtime/contracts/common.rs  (RuntimeEventType)
//   src-tauri/src/modules/runtime/stream_emitter.rs    (payload_family map)

import { strict as assert } from "node:assert";
import { describe, test } from "node:test";

// ── Mapping table: Rust backend event → TS translator coverage ──

interface WireEventMapping {
  /** Rust `RuntimeEventType` variant (snake_case wire value). */
  rustEventType: string;
  /** Canonical payload family (free-form tag on the envelope). */
  payloadFamily: string;
  /** TS translator function that handles this pair (null = unmapped). */
  translatorFunction: string | null;
  /** Why this pair is intentionally unmapped (only when translator is null). */
  exemption?: string;
}

const WIRE_EVENT_MAPPING: WireEventMapping[] = [
  // ── Conversation family ──
  {
    rustEventType: "conversation",
    payloadFamily: "text_delta",
    translatorFunction: "translateAgentTokenPayload",
  },
  {
    rustEventType: "conversation",
    payloadFamily: "thinking_start",
    translatorFunction: "translateAgentTokenPayload",
  },
  {
    rustEventType: "conversation",
    payloadFamily: "thinking_delta",
    translatorFunction: "translateAgentTokenPayload",
  },
  {
    rustEventType: "conversation",
    payloadFamily: "final_text_override",
    translatorFunction: "translateAgentTokenPayload",
  },
  {
    rustEventType: "conversation",
    payloadFamily: "skill_resolution_snapshot",
    translatorFunction: "translateAgentTokenPayload",
  },
  {
    rustEventType: "conversation",
    payloadFamily: "final_run_report",
    translatorFunction: "translateAgentTokenPayload",
  },
  {
    rustEventType: "conversation",
    payloadFamily: "stream_complete",
    translatorFunction: "translateAgentTokenPayload",
  },
  {
    rustEventType: "conversation",
    payloadFamily: "stream_error",
    translatorFunction: "translateAgentTokenPayload",
  },
  {
    rustEventType: "conversation",
    payloadFamily: "run_started",
    translatorFunction: null,
    exemption:
      "T-018 exemption: run_started carries stream_run_bound metadata only; UI does not need a dedicated surface yet",
  },

  // ── Tool family ──
  {
    rustEventType: "tool",
    payloadFamily: "tool_call_update",
    translatorFunction: "translateAgentTokenPayload",
  },

  // ── Permission family ──
  {
    rustEventType: "permission",
    payloadFamily: "permission_request",
    translatorFunction: "translatePermissionRequestPayload",
  },
  {
    rustEventType: "permission",
    payloadFamily: "permission_decision",
    translatorFunction: null,
    exemption:
      "T-018 exemption: permission_decision is a server-side record; frontend receives the prompt and resolves locally via PermissionResolvedEvent (UI-driven)",
  },

  // ── Memory family ──
  {
    rustEventType: "memory",
    payloadFamily: "memory_write_decision",
    translatorFunction: "translateMemoryWriteDecision",
  },
  {
    rustEventType: "memory",
    payloadFamily: "memory_after_turn",
    translatorFunction: "translateMemoryAfterTurn",
  },
  {
    rustEventType: "memory",
    payloadFamily: "*",
    translatorFunction: "translateMemoryEventPayload",
  },

  // ── Activation family ──
  {
    rustEventType: "activation",
    payloadFamily: "activation_status_changed",
    translatorFunction: "translateActivationSnapshot",
  },

  // ── ExecutionMode family ──
  {
    rustEventType: "execution_mode",
    payloadFamily: "execution_mode_decision",
    translatorFunction: "translateExecutionModeDecision",
  },

  // ── Harness family (M4 phase, no frontend projection yet) ──
  {
    rustEventType: "harness",
    payloadFamily: "harness_recording",
    translatorFunction: null,
    exemption:
      "T-018 exemption: harness events belong to M4 evaluation phase; no chat-UI projection needed",
  },
  {
    rustEventType: "harness",
    payloadFamily: "harness_eval",
    translatorFunction: null,
    exemption:
      "T-018 exemption: harness events belong to M4 evaluation phase; no chat-UI projection needed",
  },

  // ── Supervisor (fetch seam via IPC, not Tauri event stream) ──
  {
    rustEventType: "system",
    payloadFamily: "supervisor_snapshot",
    translatorFunction: "translateSupervisorSnapshot",
  },

  // ── System family (boot/session lifecycle, no chat projection) ──
  {
    rustEventType: "system",
    payloadFamily: "boot_phase_changed",
    translatorFunction: null,
    exemption:
      "T-018 exemption: system boot events are consumed by boot shell directly, not via projection pipeline",
  },
  {
    rustEventType: "system",
    payloadFamily: "session_opened",
    translatorFunction: null,
    exemption:
      "T-018 exemption: session lifecycle events are managed by SessionManager; not projected into runtime store",
  },
  {
    rustEventType: "system",
    payloadFamily: "session_closed",
    translatorFunction: null,
    exemption:
      "T-018 exemption: session lifecycle events are managed by SessionManager; not projected into runtime store",
  },
];

// ── Known TS translator functions (source of truth) ──

const KNOWN_TRANSLATOR_FUNCTIONS = [
  "translateAgentTokenPayload",
  "translatePermissionRequestPayload",
  "translateMemoryEventPayload",
  "translateMemoryWriteDecision",
  "translateMemoryAfterTurn",
  "translateActivationSnapshot",
  "translateExecutionModeDecision",
  "translateSupervisorSnapshot",
] as const;

// ── Test ──

describe("contract drift guardrail — wire event coverage", () => {
  test("every non-exempt wire event pair has a known translator function", () => {
    const unmapped: WireEventMapping[] = [];

    for (const mapping of WIRE_EVENT_MAPPING) {
      if (mapping.translatorFunction === null) {
        // Exempt — intentionally unmapped with documented reason
        assert.ok(
          mapping.exemption,
          `unmapped event ${mapping.rustEventType}/${mapping.payloadFamily} must have an exemption reason`,
        );
        continue;
      }

      if (
        !KNOWN_TRANSLATOR_FUNCTIONS.includes(
          mapping.translatorFunction as (typeof KNOWN_TRANSLATOR_FUNCTIONS)[number],
        )
      ) {
        unmapped.push(mapping);
      }
    }

    if (unmapped.length > 0) {
      const lines = unmapped.map(
        (m) =>
          `  ❌ ${m.rustEventType}/${m.payloadFamily} → ${m.translatorFunction} (NOT FOUND in translator registry)`,
      );
      throw new Error(
        `Contract drift detected — ${unmapped.length} wire event pair(s) reference unknown translator functions:\n${lines.join("\n")}\n\n` +
          `Add the missing function(s) to KNOWN_TRANSLATOR_FUNCTIONS in this test file, or add them to the runtime-event-translator.ts module.`,
      );
    }
  });

  test("every known translator function is referenced by at least one mapping", () => {
    const referenced = new Set(
      WIRE_EVENT_MAPPING.filter((m) => m.translatorFunction !== null).map(
        (m) => m.translatorFunction!,
      ),
    );

    const unreferenced = KNOWN_TRANSLATOR_FUNCTIONS.filter(
      (fn) => !referenced.has(fn),
    );

    if (unreferenced.length > 0) {
      throw new Error(
        `Orphan translator function(s) with no wire event mapping:\n` +
          unreferenced.map((fn) => `  ⚠ ${fn}`).join("\n") +
          `\n\nAdd the missing mapping row(s) to WIRE_EVENT_MAPPING, or remove the function from KNOWN_TRANSLATOR_FUNCTIONS.`,
      );
    }
  });

  test("no duplicate wire event pairs in the mapping table", () => {
    const seen = new Set<string>();
    const duplicates: string[] = [];

    for (const mapping of WIRE_EVENT_MAPPING) {
      const key = `${mapping.rustEventType}:${mapping.payloadFamily}`;
      if (seen.has(key)) {
        duplicates.push(key);
      }
      seen.add(key);
    }

    assert.deepEqual(duplicates, []);
  });

  test("all Rust RuntimeEventType families have coverage", () => {
    // The canonical Rust enum variants (from common.rs).
    // When a new variant is added, it MUST appear in WIRE_EVENT_MAPPING
    // or this test will remind the author.
    const RUST_EVENT_TYPE_FAMILIES = [
      "conversation",
      "tool",
      "permission",
      "memory",
      "activation",
      "execution_mode",
      "harness",
      "system",
    ] as const;

    const covered = new Set(
      WIRE_EVENT_MAPPING.map((m) => m.rustEventType),
    );

    const uncovered = RUST_EVENT_TYPE_FAMILIES.filter(
      (family) => !covered.has(family),
    );

    assert.deepEqual(uncovered, []);
  });

  test("every exempt mapping documents a meaningful exemption", () => {
    const exemptions = WIRE_EVENT_MAPPING.filter(
      (m) => m.translatorFunction === null,
    );

    for (const mapping of exemptions) {
      assert.ok(mapping.exemption);
      assert.ok(mapping.exemption.length > 20);
      // Must start with "T-018 exemption:"
      assert.match(mapping.exemption, /^T-018 exemption:/);
    }
  });
});
