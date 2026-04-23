import { strict as assert } from "node:assert";
import { describe, it } from "node:test";

import type { Message } from "@/modules/chat/types";

import { projectConversationMessagesFromRuns } from "./chat-run-projection.ts";
import type { PromptDiagnosticsSummary, RunProjection } from "./types.ts";

describe("chat run projection", () => {
  it("renders assistant text, thinking, tool cards, and completion from projection", () => {
    const placeholder: Message = {
      id: "assistant-placeholder",
      role: "assistant",
      content: "",
      timestamp: new Date(1000),
      streamId: "run-1",
      isStreaming: true,
    };
    const legacyTool: Message = {
      id: "legacy-tool",
      role: "tool",
      content: "legacy raw result",
      timestamp: new Date(1100),
      streamId: "run-1",
      toolCallId: "tool-1",
      toolName: "memory_store",
      toolStatus: "running",
    };
    const run: RunProjection = {
      runId: "run-1",
      text: "hello from projection",
      thinking: "thinking from projection",
      thinkingStarted: true,
      sessionId: "session-1",
      status: "completed",
      taskOutcome: "completed",
      resumeAvailable: false,
      toolCalls: {
        "tool-1": {
          toolCallId: "tool-1",
          toolName: "memory_store",
          status: "completed",
          toolResult:
            '{"policy_decision":"prompt","scope":"project","reason_code":"length_prompt_threshold"}',
          firstSeenAt: 1200,
          lastUpdatedAt: 1300,
        },
      },
      memoryItems: [],
      lastUpdatedAt: 1400,
    };

    const messages = projectConversationMessagesFromRuns(
      [
        { id: "user-1", role: "user", content: "hi", timestamp: new Date(900) },
        placeholder,
        legacyTool,
      ],
      { "run-1": run },
      "session-1",
    );

    assert.equal(messages.length, 3);
    assert.equal(messages[1].role, "assistant");
    assert.equal(messages[1].content, "hello from projection");
    assert.equal(messages[1].thinking, "thinking from projection");
    assert.equal(messages[1].isStreaming, false);
    assert.equal(messages[1].statusKind, "success");

    assert.equal(messages[2].role, "tool");
    assert.equal(messages[2].id, "tool-run-1-tool-1");
    assert.equal(messages[2].content.includes("legacy raw result"), false);
    assert.equal(messages[2].policyDecision, "prompt");
    assert.equal(messages[2].memoryScope, "project");
    assert.equal(messages[2].memoryReasonCode, "length_prompt_threshold");
  });

  it("only recovers unanchored projected runs for the active session", () => {
    const ownRun: RunProjection = {
      runId: "run-own",
      sessionId: "session-1",
      text: "recovered",
      thinking: "",
      thinkingStarted: false,
      status: "completed",
      taskOutcome: "completed",
      resumeAvailable: false,
      toolCalls: {},
      memoryItems: [],
      lastUpdatedAt: 2000,
    };
    const otherRun: RunProjection = {
      ...ownRun,
      runId: "run-other",
      sessionId: "session-2",
      text: "must not leak",
      lastUpdatedAt: 2100,
    };

    const messages = projectConversationMessagesFromRuns(
      [],
      { "run-own": ownRun, "run-other": otherRun },
      "session-1",
    );

    assert.equal(messages.length, 1);
    assert.equal(messages[0].streamId, "run-own");
    assert.equal(messages[0].content, "recovered");
  });

  it("maps failed and recoverable runs into visible error metadata", () => {
    const failedRun: RunProjection = {
      runId: "run-failed",
      sessionId: "session-1",
      text: "",
      thinking: "",
      thinkingStarted: false,
      status: "failed",
      taskOutcome: "partial_success",
      degradedReason: "network disconnected",
      resumeAvailable: true,
      resumeCursor: "cursor-1",
      toolCalls: {},
      memoryItems: [],
      lastUpdatedAt: 3000,
    };

    const messages = projectConversationMessagesFromRuns(
      [],
      { "run-failed": failedRun },
      "session-1",
    );

    assert.equal(messages.length, 1);
    assert.equal(messages[0].isError, true);
    assert.equal(messages[0].toolArgs?.rawError, "network disconnected");
    assert.equal(messages[0].toolArgs?.taskOutcome, "partial_success");
    assert.equal(messages[0].toolArgs?.resumeCursor, "cursor-1");
    assert.equal(messages[0].resumeAvailable, true);
  });

  it("projects prompt diagnostics from the completed run", () => {
    const diagnostics: PromptDiagnosticsSummary = {
      traceId: "trace-1",
      blockCount: 1,
      activeLaneCount: 1,
      laneSummaries: [],
      activatedEntryIds: ["entry-1"],
      activatedEntries: [],
      suppressedEntryIds: [],
      suppressedEntries: [],
      activationReasonCodes: ["scenario_match"],
      activationReasons: [],
    };
    const run: RunProjection = {
      runId: "run-diagnostics",
      sessionId: "session-1",
      text: "done",
      thinking: "",
      thinkingStarted: false,
      status: "completed",
      taskOutcome: "completed",
      resumeAvailable: false,
      toolCalls: {},
      memoryItems: [],
      promptDiagnostics: diagnostics,
      lastUpdatedAt: 4000,
    };

    const messages = projectConversationMessagesFromRuns(
      [],
      { "run-diagnostics": run },
      "session-1",
    );

    assert.equal(messages[0].promptDiagnostics, diagnostics);
  });

  it("interleaves recovered run messages after their bound user messages", () => {
    const firstRun: RunProjection = {
      runId: "run-first",
      sessionId: "session-1",
      text: "first answer",
      thinking: "",
      thinkingStarted: false,
      status: "completed",
      taskOutcome: "completed",
      resumeAvailable: false,
      toolCalls: {},
      memoryItems: [],
      lastUpdatedAt: 3000,
    };
    const secondRun: RunProjection = {
      ...firstRun,
      runId: "run-second",
      text: "second answer",
      lastUpdatedAt: 2000,
    };

    const messages = projectConversationMessagesFromRuns(
      [
        {
          id: "user-first",
          role: "user",
          content: "first question",
          timestamp: new Date(1000),
          streamId: "run-first",
        },
        {
          id: "user-second",
          role: "user",
          content: "second question",
          timestamp: new Date(1500),
          streamId: "run-second",
        },
      ],
      { "run-first": firstRun, "run-second": secondRun },
      "session-1",
    );

    assert.deepEqual(
      messages.map((message) => message.content),
      ["first question", "first answer", "second question", "second answer"],
    );
  });

  it("inserts unanchored recovered runs before later anchored runs", () => {
    const earlierRecoveredRun: RunProjection = {
      runId: "run-earlier",
      sessionId: "session-1",
      text: "earlier answer",
      thinking: "",
      thinkingStarted: false,
      status: "completed",
      taskOutcome: "completed",
      resumeAvailable: false,
      toolCalls: {},
      memoryItems: [],
      lastUpdatedAt: 900,
    };
    const laterAnchoredRun: RunProjection = {
      ...earlierRecoveredRun,
      runId: "run-later",
      text: "later answer",
      lastUpdatedAt: 3000,
    };

    const messages = projectConversationMessagesFromRuns(
      [
        {
          id: "user-later",
          role: "user",
          content: "later question",
          timestamp: new Date(2000),
          streamId: "run-later",
        },
      ],
      { "run-earlier": earlierRecoveredRun, "run-later": laterAnchoredRun },
      "session-1",
    );

    assert.deepEqual(
      messages.map((message) => message.content),
      ["earlier answer", "later question", "later answer"],
    );
  });

  it("does not duplicate a run when both user and assistant placeholders are present", () => {
    const run: RunProjection = {
      runId: "run-1",
      sessionId: "session-1",
      text: "projected answer",
      thinking: "",
      thinkingStarted: false,
      status: "completed",
      taskOutcome: "completed",
      resumeAvailable: false,
      toolCalls: {},
      memoryItems: [],
      lastUpdatedAt: 3000,
    };

    const messages = projectConversationMessagesFromRuns(
      [
        {
          id: "user-1",
          role: "user",
          content: "question",
          timestamp: new Date(1000),
          streamId: "run-1",
        },
        {
          id: "assistant-placeholder",
          role: "assistant",
          content: "",
          timestamp: new Date(2000),
          streamId: "run-1",
          isStreaming: true,
        },
      ],
      { "run-1": run },
      "session-1",
    );

    assert.deepEqual(
      messages.map((message) => message.content),
      ["question", "projected answer"],
    );
  });
});
