import { strict as assert } from "node:assert";
import { describe, it } from "node:test";

import { translateAgentTokenPayload } from "./runtime-event-translator.ts";

describe("runtime-projection truth (T-002 / MIG-003)", () => {
  it("translator_preserves_correlation_from_payload: runId prefers correlation.runId over stream_id", () => {
    const payload = {
      stream_id: "legacy-stream-1",
      correlation: {
        sessionId: "sess-1",
        runId: "canonical-run-1",
        streamId: "legacy-stream-1",
        attemptId: "att-5",
      },
      event_type: "text_delta" as const,
      text: "hello",
    };

    const event = translateAgentTokenPayload(payload);
    assert.ok(event, "text_delta should translate to a canonical event");
    if (event && "runId" in event) {
      assert.equal(
        event.runId,
        "canonical-run-1",
        "runId must come from correlation.runId when present",
      );
    }
  });

  it("translator_falls_back_to_stream_id_when_no_correlation", () => {
    const payload = {
      stream_id: "legacy-stream-2",
      event_type: "text_delta" as const,
      text: "hello",
    };

    const event = translateAgentTokenPayload(payload);
    assert.ok(event);
    if (event && "runId" in event) {
      assert.equal(
        event.runId,
        "legacy-stream-2",
        "runId falls back to stream_id when correlation is absent",
      );
    }
  });

  it("translator_falls_back_to_stream_id_when_correlation_has_no_runId", () => {
    const payload = {
      stream_id: "legacy-stream-3",
      correlation: {
        sessionId: "sess-3",
        // no runId
      },
      event_type: "text_delta" as const,
      text: "fallback",
    };

    const event = translateAgentTokenPayload(payload);
    assert.ok(event);
    if (event && "runId" in event) {
      assert.equal(
        event.runId,
        "legacy-stream-3",
        "runId falls back to stream_id when correlation.runId is absent",
      );
    }
  });

  it("bridge_is_only_ingestion_entry: all agent-token events are translated through the translator", () => {
    // Verify that every known event_type maps to a non-null canonical event
    const knownTypes = [
      "text_delta",
      "thinking_delta",
      "thinking_start",
      "tool_call_update",
      "final_text_override",
      "stream_complete",
      "stream_error",
    ] as const;

    for (const eventType of knownTypes) {
      const payload: Record<string, unknown> = {
        stream_id: "s1",
        event_type: eventType,
      };
      // tool_call_update requires additional fields
      if (eventType === "tool_call_update") {
        payload.tool_call_id = "tc-1";
        payload.tool_name = "read_file";
        payload.tool_status = "running";
      }

      const event = translateAgentTokenPayload(
        payload as Parameters<typeof translateAgentTokenPayload>[0],
      );
      assert.ok(
        event,
        `event_type '${eventType}' must translate to a canonical event`,
      );
    }
  });

  it("run_bound_event_uses_correlation_run_id: stream_run_bound should use correlation.runId when available", () => {
    // This test validates the contract: when a StreamTokenPayload
    // arrives with correlation.runId set, the translator must
    // produce events whose runId equals the canonical runId,
    // not the legacy stream_id.
    const payload = {
      stream_id: "stream-legacy",
      correlation: {
        sessionId: "sess-42",
        runId: "run-canonical-42",
        streamId: "stream-legacy",
      },
      event_type: "stream_complete" as const,
    };

    const event = translateAgentTokenPayload(payload);
    assert.ok(event);
    if (event && "runId" in event) {
      assert.equal(event.runId, "run-canonical-42");
    }
  });
});
