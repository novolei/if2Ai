import { strict as assert } from "node:assert";
import { describe, it } from "node:test";

import type { RunLogEntry } from "@/api/sessions";

import { replayRunLogEntriesToMessages } from "./history-replay.ts";

function entry(
  seq: number,
  eventType: string,
  payload: Record<string, unknown>,
): RunLogEntry {
  return {
    event_id: `event-${seq}`,
    session_id: "session-1",
    run_id: "run-1",
    seq,
    event_type: eventType,
    occurred_at: `2026-04-23T00:00:0${seq}.000Z`,
    payload,
  };
}

describe("history-replay", () => {
  it("replays user, assistant, thinking, tool result, and completion", () => {
    const messages = replayRunLogEntriesToMessages(
      [
        entry(1, "run_started", { message_preview: "hello" }),
        entry(2, "thinking_delta", { thinking: "plan" }),
        entry(3, "text_delta", { text: "answer" }),
        entry(4, "tool_call_completed", {
          tool_call_id: "tool-1",
          tool_name: "memory_store",
          tool_status: "completed",
          tool_result:
            '{"policy_decision":"allow","scope":"session","reason_code":"accepted"}',
        }),
        entry(5, "stream_complete", { task_outcome: "completed" }),
      ],
      "session-1",
    );

    assert.equal(messages.length, 3);
    assert.equal(messages[0].role, "user");
    assert.equal(messages[0].content, "hello");
    assert.equal(messages[1].role, "assistant");
    assert.equal(messages[1].content, "answer");
    assert.equal(messages[1].thinking, "plan");
    assert.equal(messages[1].statusKind, "success");
    assert.equal(messages[2].role, "tool");
    assert.equal(messages[2].content.includes("policy_decision"), true);
    assert.equal(messages[2].policyDecision, "allow");
    assert.equal(messages[2].memoryScope, "session");
    assert.equal(messages[2].memoryReasonCode, "accepted");
  });

  it("replays the same event batch stably", () => {
    const events = [
      entry(1, "run_started", { message_preview: "hello" }),
      entry(2, "text_delta", { text: "same" }),
      entry(3, "stream_complete", { task_outcome: "completed" }),
    ];

    assert.deepEqual(
      replayRunLogEntriesToMessages(events, "session-1"),
      replayRunLogEntriesToMessages(events, "session-1"),
    );
  });

  it("replays a page that starts in the middle of a run", () => {
    const messages = replayRunLogEntriesToMessages(
      [
        entry(2, "text_delta", { text: "continued" }),
        entry(3, "stream_complete", { task_outcome: "completed" }),
      ],
      "session-1",
    );

    assert.equal(messages.length, 1);
    assert.equal(messages[0].role, "assistant");
    assert.equal(messages[0].content, "continued");
    assert.equal(messages[0].statusKind, "success");
  });
});
