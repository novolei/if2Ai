import type { Message } from "@/modules/chat/types";

import type { RunProjection, ToolCallProjection } from "./types.ts";

type MemoryStoreFields = Pick<
  Message,
  "policyDecision" | "memoryScope" | "memoryReasonCode"
>;

/** Parse structured memory_store output into chat-card metadata. */
export function extractMemoryStoreFields(
  raw: string | null | undefined,
): MemoryStoreFields | null {
  if (!raw) return null;
  let parsed: Record<string, unknown>;
  try {
    const candidate = JSON.parse(raw);
    if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
      return null;
    }
    parsed = candidate as Record<string, unknown>;
  } catch {
    return null;
  }

  const fields: MemoryStoreFields = {};
  const decision = parsed.policy_decision;
  if (decision === "allow" || decision === "deny" || decision === "prompt") {
    fields.policyDecision = decision;
  }

  const scope = parsed.scope;
  if (scope === "global" || scope === "project" || scope === "session") {
    fields.memoryScope = scope;
  }

  if (typeof parsed.reason_code === "string") {
    fields.memoryReasonCode = parsed.reason_code;
  }

  return Object.keys(fields).length > 0 ? fields : null;
}

/** Replace streaming placeholders with canonical projection messages.
 *
 * ## T-003 (MIG-017) Chat Truth Cutover
 *
 * The `messages` parameter MUST only contain **user-role** messages
 * (and may include a filter-out for `[resume_cursor]` markers).
 * Assistant / tool / thinking / completion messages are derived
 * entirely from the canonical [`RunProjection`] store — the
 * conversation slice is no longer a source of runtime transcript
 * truth.
 *
 * Each user message carries a `streamId` that anchors its
 * corresponding run.  Unanchored runs (e.g. recovered from a
 * projection checkpoint) are inserted by timestamp.
 */
export function projectConversationMessagesFromRuns(
  messages: readonly Message[],
  runs: Record<string, RunProjection>,
  sessionId?: string | null,
): Message[] {
  const projectedRunIds = new Set<string>();
  const next: Message[] = [];

  for (const message of messages) {
    const runId = message.streamId;
    const run = runId ? runs[runId] : undefined;
    if (message.role === "assistant" && run && runId) {
      if (projectedRunIds.has(runId)) {
        continue;
      }
      projectedRunIds.add(runId);
      next.push(...projectRunToChatMessages(message, run));
      continue;
    }
    if (message.role === "user" && run && runId) {
      projectedRunIds.add(runId);
      next.push(message);
      next.push(...projectRunToChatMessages(undefined, run));
      continue;
    }
    if (message.role === "tool" && run) {
      continue;
    }
    next.push(message);
  }

  for (const run of Object.values(runs).sort(byRunUpdate)) {
    if (
      sessionId &&
      run.sessionId === sessionId &&
      !projectedRunIds.has(run.runId)
    ) {
      insertMessagesByTimestamp(next, projectRunToChatMessages(undefined, run));
    }
  }

  return next;
}

function insertMessagesByTimestamp(
  messages: Message[],
  recoveredMessages: Message[],
): void {
  if (recoveredMessages.length === 0) return;
  const firstTimestamp = recoveredMessages[0].timestamp.getTime();
  const insertionIndex = messages.findIndex(
    (message) => message.timestamp.getTime() > firstTimestamp,
  );
  if (insertionIndex === -1) {
    messages.push(...recoveredMessages);
    return;
  }
  messages.splice(insertionIndex, 0, ...recoveredMessages);
}

/** Convert one run projection into the chat UI message family. */
export function projectRunToChatMessages(
  placeholder: Message | undefined,
  run: RunProjection,
): Message[] {
  const timestamp = placeholder?.timestamp ?? new Date(run.lastUpdatedAt);
  const assistant: Message = {
    ...(placeholder ?? {}),
    id: placeholder?.id ?? `assistant-${run.runId}`,
    role: "assistant",
    content: run.text,
    thinking: run.thinking || placeholder?.thinking,
    timestamp,
    streamId: run.runId,
    isStreaming: run.status === "streaming",
    isError: run.status === "failed" || placeholder?.isError,
    taskOutcome: run.taskOutcome ?? placeholder?.taskOutcome,
    degradedReason: run.degradedReason ?? placeholder?.degradedReason,
    resumeAvailable: run.resumeAvailable,
    resumeCursor: run.resumeCursor ?? placeholder?.resumeCursor,
    memoryContext: run.memoryItems.length > 0 ? run.memoryItems : placeholder?.memoryContext,
    contextBudgetUsage: run.contextBudgetUsage ?? placeholder?.contextBudgetUsage,
    promptDiagnostics: run.promptDiagnostics ?? placeholder?.promptDiagnostics,
    turnCost: run.turnCost ?? placeholder?.turnCost,
    routing: run.routing ?? placeholder?.routing,
    toolArgs: buildRunErrorToolArgs(run, placeholder),
    statusLabel:
      run.status === "streaming"
        ? placeholder?.statusLabel
        : buildRunCompletionStatus(run).label,
    statusKind:
      run.status === "streaming"
        ? placeholder?.statusKind
        : buildRunCompletionStatus(run).kind,
  };

  const tools = Object.values(run.toolCalls)
    .sort(byToolUpdate)
    .map((tool) => projectToolCallToMessage(run, tool));

  return [assistant, ...tools];
}

function buildRunErrorToolArgs(
  run: RunProjection,
  placeholder: Message | undefined,
): Record<string, unknown> | undefined {
  const failed = run.status === "failed" || run.taskOutcome === "failed";
  const recoverable = run.taskOutcome === "partial_success" && run.resumeAvailable;
  if (!failed && !recoverable) {
    return placeholder?.toolArgs;
  }

  return {
    ...(placeholder?.toolArgs ?? {}),
    rawError:
      run.degradedReason ??
      placeholder?.degradedReason ??
      placeholder?.toolArgs?.rawError ??
      "Agent 执行失败，请稍后重试。",
    taskOutcome: run.taskOutcome ?? (failed ? "failed" : undefined),
    degradedReason: run.degradedReason ?? placeholder?.degradedReason,
    resumeCursor: run.resumeCursor ?? placeholder?.resumeCursor,
  };
}

function projectToolCallToMessage(
  run: RunProjection,
  tool: ToolCallProjection,
): Message {
  const terminalRun = run.status !== "streaming";
  const unfinished = tool.status === "queued" || tool.status === "running";
  const status = terminalRun && unfinished ? "error" : tool.status;
  const memoryFields =
    tool.toolName === "memory_store"
      ? extractMemoryStoreFields(tool.toolResult)
      : null;

  return {
    id: `tool-${run.runId}-${tool.toolCallId}`,
    role: "tool",
    content:
      status === "queued" || status === "running"
        ? ""
        : tool.toolResult ||
          (unfinished ? "stream completed before tool reached terminal state" : ""),
    timestamp: new Date(tool.firstSeenAt),
    disableAnimation: true,
    streamId: run.runId,
    toolCallId: tool.toolCallId,
    toolName: tool.toolName,
    toolArgs: tool.toolArgs,
    toolDurationMs: tool.toolDurationMs,
    toolStatus: status,
    isError: status === "error",
    effectiveWorkdir: tool.effectiveWorkdir,
    policyDecision: memoryFields?.policyDecision ?? tool.policyDecision,
    memoryScope: memoryFields?.memoryScope,
    memoryReasonCode: memoryFields?.memoryReasonCode,
    evidenceId: tool.evidenceId,
    requestId: tool.requestId,
    taskOutcome: run.taskOutcome,
    degradedReason: run.degradedReason,
    resumeAvailable: run.resumeAvailable,
    resumeCursor: run.resumeCursor,
  };
}

function buildRunCompletionStatus(
  run: RunProjection,
): { label: string | undefined; kind: Message["statusKind"] | undefined } {
  const tools = Object.values(run.toolCalls);
  const total = tools.length;
  const completed = tools.filter((tool) => tool.status === "completed").length;
  const failed = tools.filter((tool) => tool.status === "error").length;

  if (run.status === "failed" || run.taskOutcome === "failed") {
    return {
      label: total > 0 ? `本轮执行失败：已完成 ${completed}/${total} 个步骤` : undefined,
      kind: "failed",
    };
  }

  if (run.taskOutcome === "partial_success") {
    return {
      label:
        total > 0
          ? `本轮部分完成：已完成 ${completed}/${total} 个步骤`
          : "本轮任务部分完成，可继续补全",
      kind: "partial",
    };
  }

  if (run.status !== "completed") {
    return { label: undefined, kind: undefined };
  }

  if (total === 0) {
    return { label: "本轮执行完成", kind: "success" };
  }

  if (failed > 0) {
    return {
      label: `本轮执行完成：成功 ${completed} 个，失败 ${failed} 个`,
      kind: "partial",
    };
  }

  return {
    label: `本轮执行完成：共完成 ${completed} 个步骤`,
    kind: "success",
  };
}

function byToolUpdate(left: ToolCallProjection, right: ToolCallProjection): number {
  return left.firstSeenAt - right.firstSeenAt;
}

function byRunUpdate(left: RunProjection, right: RunProjection): number {
  return left.lastUpdatedAt - right.lastUpdatedAt;
}
