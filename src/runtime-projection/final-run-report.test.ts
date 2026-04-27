import { strict as assert } from "node:assert";
import { describe, it } from "node:test";

import type { FinalRunReport, StreamTokenPayload } from "@/transport/contracts";

import { projectConversationMessagesFromRuns } from "./chat-run-projection.ts";
import { reduceRuntimeEventBatch } from "./runtime-event-reducer.ts";
import { translateAgentTokenPayload } from "./runtime-event-translator.ts";
import { emptyProjectionSnapshot } from "./types.ts";

function report(overrides: Partial<FinalRunReport> = {}): FinalRunReport {
  return {
    loopKind: "autonomous_work",
    outcome: "completed",
    taskOutcome: "completed",
    terminalStatus: "model_stop",
    requestId: "req-1",
    toolLoopIterations: 1,
    hasSuccessfulTool: true,
    hasSuccessfulMutatingTool: false,
    resumeAvailable: false,
    completedItems: ["Inspected the requested files."],
    failedItems: [],
    userNextSteps: ["Review the result."],
    loadedSkills: ["rust-helper"],
    blockedSkills: [],
    skillWarnings: [],
    ...overrides,
  };
}

function finalReportEvent(runId: string, finalReport: FinalRunReport) {
  const payload = {
    stream_id: runId,
    correlation: { runId, sessionId: "session-1" },
    event_type: "final_run_report",
    tool_args: finalReport,
  } satisfies StreamTokenPayload;
  const event = translateAgentTokenPayload(payload);
  assert.ok(event);
  assert.equal(event.kind, "final_run_report");
  return event;
}

describe("final run report projection", () => {
  it("stores loaded and blocked skill evidence from the canonical final report", () => {
    const snapshot = reduceRuntimeEventBatch(emptyProjectionSnapshot(), [
      {
        kind: "stream_run_bound",
        runId: "run-1",
        sessionId: "session-1",
        receivedAt: 1,
      },
      finalReportEvent(
        "run-1",
        report({
          blockedSkills: ["remote-helper"],
          skillWarnings: ["remote-helper blocked"],
        }),
      ),
    ]);

    const stored = snapshot.runs["run-1"]?.finalRunReport;
    assert.ok(stored);
    assert.deepEqual(stored.loadedSkills, ["rust-helper"]);
    assert.deepEqual(stored.blockedSkills, ["remote-helper"]);
    assert.deepEqual(stored.skillWarnings, ["remote-helper blocked"]);
  });

  it("projects success report data into the assistant transcript message", () => {
    const snapshot = reduceRuntimeEventBatch(emptyProjectionSnapshot(), [
      {
        kind: "stream_run_bound",
        runId: "run-success",
        sessionId: "session-1",
        receivedAt: 1,
      },
      finalReportEvent("run-success", report()),
    ]);

    const messages = projectConversationMessagesFromRuns(
      [],
      snapshot.runs,
      "session-1",
    );

    assert.equal(messages.length, 1);
    assert.equal(messages[0].finalRunReport?.outcome, "completed");
    assert.deepEqual(messages[0].finalRunReport?.completedItems, [
      "Inspected the requested files.",
    ]);
    assert.deepEqual(messages[0].finalRunReport?.loadedSkills, ["rust-helper"]);
  });

  it("projects provider failure next steps without recomputing report truth", () => {
    const failure = report({
      outcome: "failed_with_plan",
      taskOutcome: "failed",
      terminalStatus: "provider_prepare_failed",
      completedItems: [],
      failedItems: ["Run ended with status `provider_prepare_failed`."],
      userNextSteps: ["Retry after addressing the reported failure."],
      loadedSkills: [],
    });
    const snapshot = reduceRuntimeEventBatch(emptyProjectionSnapshot(), [
      {
        kind: "stream_run_bound",
        runId: "run-failure",
        sessionId: "session-1",
        receivedAt: 1,
      },
      finalReportEvent("run-failure", failure),
    ]);

    const messages = projectConversationMessagesFromRuns(
      [],
      snapshot.runs,
      "session-1",
    );

    assert.equal(messages[0].statusKind, "failed");
    assert.equal(messages[0].finalRunReport?.terminalStatus, "provider_prepare_failed");
    assert.deepEqual(messages[0].finalRunReport?.failedItems, failure.failedItems);
    assert.deepEqual(messages[0].finalRunReport?.userNextSteps, failure.userNextSteps);
  });

  it("projects approval-blocked and budget-exhausted report guidance", () => {
    const approval = report({
      outcome: "needs_approval",
      terminalStatus: "approval_required_for_mutation",
      userNextSteps: ["Approve or reject the pending tool action."],
      loadedSkills: [],
    });
    const exhausted = report({
      outcome: "exhausted_with_summary",
      taskOutcome: "partial_success",
      terminalStatus: "max_iterations_reached",
      resumeAvailable: true,
      resumeCursor: "cursor-1",
      userNextSteps: ["Continue the run so the agent can proceed from existing context."],
      loadedSkills: [],
    });
    const snapshot = reduceRuntimeEventBatch(emptyProjectionSnapshot(), [
      {
        kind: "stream_run_bound",
        runId: "run-approval",
        sessionId: "session-1",
        receivedAt: 1,
      },
      finalReportEvent("run-approval", approval),
      {
        kind: "stream_run_bound",
        runId: "run-exhausted",
        sessionId: "session-1",
        receivedAt: 2,
      },
      finalReportEvent("run-exhausted", exhausted),
    ]);

    const messages = projectConversationMessagesFromRuns(
      [],
      snapshot.runs,
      "session-1",
    );
    const approvalMessage = messages.find((message) => message.streamId === "run-approval");
    const exhaustedMessage = messages.find((message) => message.streamId === "run-exhausted");

    assert.equal(approvalMessage?.statusLabel, "等待用户审批");
    assert.equal(approvalMessage?.finalRunReport?.outcome, "needs_approval");
    assert.equal(exhaustedMessage?.statusLabel, "达到执行上限，可继续恢复");
    assert.equal(exhaustedMessage?.resumeCursor, "cursor-1");
    assert.equal(exhaustedMessage?.finalRunReport?.resumeAvailable, true);
  });

  it("preserves empty skill sections so UI can omit empty report groups", () => {
    const emptySkillReport = report({
      loadedSkills: [],
      blockedSkills: [],
      skillWarnings: [],
    });
    const snapshot = reduceRuntimeEventBatch(emptyProjectionSnapshot(), [
      {
        kind: "stream_run_bound",
        runId: "run-empty-skills",
        sessionId: "session-1",
        receivedAt: 1,
      },
      finalReportEvent("run-empty-skills", emptySkillReport),
    ]);

    const message = projectConversationMessagesFromRuns(
      [],
      snapshot.runs,
      "session-1",
    )[0];

    assert.deepEqual(message.finalRunReport?.loadedSkills, []);
    assert.deepEqual(message.finalRunReport?.blockedSkills, []);
    assert.deepEqual(message.finalRunReport?.skillWarnings, []);
  });

  it("keeps legacy backend error tool status visible as a failed tool card", () => {
    const toolErrorPayload = {
      stream_id: "run-tool-error",
      correlation: { runId: "run-tool-error", sessionId: "session-1" },
      event_type: "tool_call_update",
      tool_call_id: "tool-1",
      tool_name: "write_file",
      tool_status: "error",
      tool_args: { path: "src/main.rs" },
      tool_result: "permission denied",
    } satisfies StreamTokenPayload;
    const event = translateAgentTokenPayload(toolErrorPayload);
    assert.ok(event);
    const snapshot = reduceRuntimeEventBatch(emptyProjectionSnapshot(), [
      {
        kind: "stream_run_bound",
        runId: "run-tool-error",
        sessionId: "session-1",
        receivedAt: 1,
      },
      event,
      finalReportEvent(
        "run-tool-error",
        report({
          outcome: "needs_approval",
          terminalStatus: "approval_required_for_mutation",
          loadedSkills: [],
        }),
      ),
    ]);

    const messages = projectConversationMessagesFromRuns(
      [],
      snapshot.runs,
      "session-1",
    );
    const toolMessage = messages.find((message) => message.role === "tool");

    assert.equal(toolMessage?.toolStatus, "error");
    assert.equal(toolMessage?.isError, true);
  });
});
