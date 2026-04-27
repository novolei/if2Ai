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
          attemptHistory: [],
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

  it("keeps historical assistant messages without matching projection while projecting the live run", () => {
    const liveRun: RunProjection = {
      runId: "run-live",
      sessionId: "session-1",
      text: "live projected answer",
      thinking: "",
      thinkingStarted: false,
      status: "streaming",
      resumeAvailable: false,
      toolCalls: {},
      memoryItems: [],
      lastUpdatedAt: 4000,
    };

    const messages = projectConversationMessagesFromRuns(
      [
        {
          id: "user-old",
          role: "user",
          content: "old question",
          timestamp: new Date(1000),
        },
        {
          id: "assistant-old",
          role: "assistant",
          content: "old answer must stay visible",
          timestamp: new Date(2000),
        },
        {
          id: "user-live",
          role: "user",
          content: "new question",
          timestamp: new Date(3000),
        },
        {
          id: "assistant-live-placeholder",
          role: "assistant",
          content: "",
          timestamp: new Date(3500),
          streamId: "run-live",
          isStreaming: true,
        },
      ],
      { "run-live": liveRun },
      "session-1",
    );

    assert.deepEqual(
      messages.map((message) => message.content),
      [
        "old question",
        "old answer must stay visible",
        "new question",
        "live projected answer",
      ],
    );
    assert.equal(messages[3].streamId, "run-live");
    assert.equal(messages[3].isStreaming, true);
  });

  // GAP-003 (T-005): Verify projection single truth — confirms that
  // `projectConversationMessagesFromRuns` never leaks non-user messages
  // through as-is (assistant/tool placeholders from the legacy slice are
  // replaced by projection-derived content).
  it("replaces legacy assistant/tool placeholders with projection-derived content", () => {
    const run: RunProjection = {
      runId: "run-gap003",
      sessionId: "session-1",
      text: "projection answer",
      thinking: "",
      thinkingStarted: false,
      status: "completed",
      taskOutcome: "completed",
      resumeAvailable: false,
      toolCalls: {},
      memoryItems: [],
      lastUpdatedAt: 2000,
    };

    // Legacy slice passes assistant + tool placeholders — the function
    // must replace them, not pass them through verbatim.
    const messages = projectConversationMessagesFromRuns(
      [
        {
          id: "user-gap003",
          role: "user",
          content: "q",
          timestamp: new Date(1000),
          streamId: "run-gap003",
        },
        {
          id: "legacy-asst",
          role: "assistant",
          content: "legacy content that must be replaced",
          timestamp: new Date(1500),
          streamId: "run-gap003",
          isStreaming: true,
        },
        {
          id: "legacy-tool",
          role: "tool",
          content: "legacy tool result that must be removed",
          timestamp: new Date(1600),
          streamId: "run-gap003",
          toolCallId: "legacy-tc",
          toolName: "legacy-tool",
          toolStatus: "running",
        },
      ],
      { "run-gap003": run },
      "session-1",
    );

    // Only user + projection-derived assistant (tool placeholders with no
    // matching run tool calls are dropped).
    assert.equal(messages.length, 2);
    assert.equal(messages[0].role, "user");
    assert.equal(messages[0].content, "q");
    assert.equal(messages[1].role, "assistant");
    assert.equal(messages[1].content, "projection answer");
    // Projection replaces legacy placeholders with new ids (e.g.
    // "assistant-run-gap003"), confirming the placeholder is NOT
    // passed through verbatim.
    assert.ok(messages[1].id.startsWith("assistant-"));
    assert.equal(messages[1].streamId, "run-gap003");
  });

  // T-003 (MIG-017): Verify projection-only truth — only user messages
  // are fed in; assistant/tool/thinking/completion are all derived from
  // the canonical RunProjection store.
  it("derives all assistant and tool content from projection when only user messages are passed", () => {
    const run: RunProjection = {
      runId: "run-003",
      sessionId: "session-1",
      text: "projection-only assistant text",
      thinking: "projection-only thinking",
      thinkingStarted: true,
      status: "completed",
      taskOutcome: "completed",
      resumeAvailable: false,
      toolCalls: {
        "tool-003": {
          toolCallId: "tool-003",
          toolName: "bash",
          toolArgs: { cmd: "ls" },
          status: "completed",
          toolResult: "file listing",
          firstSeenAt: 500,
          lastUpdatedAt: 600,
          attemptHistory: [],
        },
      },
      contextBudgetUsage: {
        total_budget: 32000,
        system_tokens: 5000,
        history_tokens: 10000,
        memory_tokens: 300,
        output_reserve: 4096,
        remaining: 12604,
      },
      memoryItems: [],
      lastUpdatedAt: 700,
    };

    // Only user messages — no assistant/tool placeholders
    const messages = projectConversationMessagesFromRuns(
      [
        {
          id: "user-003",
          role: "user",
          content: "list files",
          timestamp: new Date(400),
          streamId: "run-003",
        },
      ],
      { "run-003": run },
      "session-1",
    );

    // 3 messages: user, assistant, tool
    assert.equal(messages.length, 3);
    assert.equal(messages[0].role, "user");
    assert.equal(messages[0].content, "list files");

    assert.equal(messages[1].role, "assistant");
    assert.equal(messages[1].content, "projection-only assistant text");
    assert.equal(messages[1].thinking, "projection-only thinking");
    assert.equal(messages[1].isStreaming, false);
    assert.equal(messages[1].streamId, "run-003");

    assert.equal(messages[2].role, "tool");
    assert.equal(messages[2].toolName, "bash");
    assert.equal(messages[2].content, "file listing");
    assert.equal(messages[2].toolStatus, "completed");
    // toolArgs is populated by projectToolCallToMessage
    const toolMsg = messages[2] as any;
    assert.equal(toolMsg.toolArgs?.cmd, "ls");
  });
});
