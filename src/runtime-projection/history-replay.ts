import type { Message } from "@/modules/chat/types";
import type { RunLogEntry } from "@/api/sessions";

import { projectConversationMessagesFromRuns } from "./chat-run-projection.ts";
import { reduceRuntimeEvent } from "./runtime-event-reducer.ts";
import { emptyProjectionSnapshot } from "./types.ts";
import type { CanonicalRuntimeEvent, ToolCallStatus } from "./types.ts";

/** Replay canonical run-log entries into chat messages. */
export function replayRunLogEntriesToMessages(
  entries: readonly RunLogEntry[],
  sessionId: string,
): Message[] {
  let snapshot = emptyProjectionSnapshot();
  const baseMessages: Message[] = [];
  const boundRunIds = new Set<string>();

  for (const entry of entries) {
    if (entry.event_type === "run_started") {
      baseMessages.push({
        id: `history-user-${entry.run_id}`,
        role: "user",
        content: stringField(entry.payload, "message_preview") ?? "",
        timestamp: new Date(entry.occurred_at),
        streamId: entry.run_id,
      });
    }

    const event = runLogEntryToRuntimeEvent(entry);
    if (!boundRunIds.has(entry.run_id) && event?.kind !== "stream_run_bound") {
      snapshot = reduceRuntimeEvent(snapshot, {
        kind: "stream_run_bound",
        runId: entry.run_id,
        sessionId: entry.session_id,
        receivedAt: timestampFromEntry(entry),
      });
      boundRunIds.add(entry.run_id);
    }
    if (event) {
      snapshot = reduceRuntimeEvent(snapshot, event);
      if (event.kind === "stream_run_bound") {
        boundRunIds.add(entry.run_id);
      }
    }
  }

  return projectConversationMessagesFromRuns(
    baseMessages,
    snapshot.runs,
    sessionId,
  );
}

function timestampFromEntry(entry: RunLogEntry): number {
  const receivedAt = Date.parse(entry.occurred_at);
  return Number.isFinite(receivedAt) ? receivedAt : 0;
}

/** Translate one durable event-log entry back into reducer input. */
export function runLogEntryToRuntimeEvent(
  entry: RunLogEntry,
): CanonicalRuntimeEvent | null {
  const timestamp = timestampFromEntry(entry);

  switch (entry.event_type) {
    case "run_started":
      return {
        kind: "stream_run_bound",
        runId: entry.run_id,
        sessionId: entry.session_id,
        receivedAt: timestamp,
      };
    case "text_delta":
      return {
        kind: "stream_text_delta",
        runId: entry.run_id,
        text: stringField(entry.payload, "text") ?? "",
        receivedAt: timestamp,
      };
    case "thinking_started":
    case "thinking_start":
      return {
        kind: "stream_thinking_start",
        runId: entry.run_id,
        receivedAt: timestamp,
      };
    case "thinking_delta":
      return {
        kind: "stream_thinking_delta",
        runId: entry.run_id,
        thinking: stringField(entry.payload, "thinking") ?? "",
        receivedAt: timestamp,
      };
    case "tool_call_queued":
    case "tool_call_running":
    case "tool_call_completed":
    case "tool_call_failed":
    case "tool_call_update":
      return toolEvent(entry, timestamp);
    case "final_text_override":
      return {
        kind: "stream_final_text_override",
        runId: entry.run_id,
        text: stringField(entry.payload, "text") ?? "",
        requestId: stringField(entry.payload, "request_id"),
        receivedAt: timestamp,
      };
    case "stream_complete":
      return {
        kind: "stream_complete",
        runId: entry.run_id,
        taskOutcome: taskOutcomeField(entry.payload, "task_outcome"),
        degradedReason: stringField(entry.payload, "degraded_reason"),
        resumeAvailable: booleanField(entry.payload, "resume_available"),
        resumeCursor: stringField(entry.payload, "resume_cursor"),
        requestId: stringField(entry.payload, "request_id"),
        receivedAt: timestamp,
      };
    case "stream_error":
      return {
        kind: "stream_error",
        runId: entry.run_id,
        reason:
          stringField(entry.payload, "tool_result") ??
          stringField(entry.payload, "degraded_reason") ??
          stringField(entry.payload, "reason") ??
          "unknown stream error",
        taskOutcome: taskOutcomeField(entry.payload, "task_outcome"),
        degradedReason: stringField(entry.payload, "degraded_reason"),
        resumeAvailable: booleanField(entry.payload, "resume_available"),
        resumeCursor: stringField(entry.payload, "resume_cursor"),
        requestId: stringField(entry.payload, "request_id"),
        receivedAt: timestamp,
      };
    default:
      return null;
  }
}

function toolEvent(
  entry: RunLogEntry,
  receivedAt: number,
): CanonicalRuntimeEvent | null {
  const toolCallId =
    stringField(entry.payload, "tool_call_id") ??
    entry.tool_call_id ??
    `tool-${entry.seq}`;
  const toolName = stringField(entry.payload, "tool_name");
  if (!toolName) return null;

  return {
    kind: "stream_tool_call_update",
    runId: entry.run_id,
    toolCallId,
    toolName,
    status:
      (toolStatusField(entry.payload, "tool_status") ??
        statusFromEventType(entry.event_type)) as ToolCallStatus,
    toolArgs: objectField(entry.payload, "tool_args"),
    toolResult: stringField(entry.payload, "tool_result"),
    toolDurationMs: numberField(entry.payload, "tool_duration_ms"),
    effectiveWorkdir: stringField(entry.payload, "effective_workdir"),
    policyDecision: policyDecisionField(entry.payload, "policy_decision"),
    evidenceId: stringField(entry.payload, "evidence_id"),
    requestId: stringField(entry.payload, "request_id"),
    receivedAt,
  };
}

function statusFromEventType(eventType: string): ToolCallStatus | "error" {
  if (eventType === "tool_call_queued") return "queued";
  if (eventType === "tool_call_running") return "running";
  if (eventType === "tool_call_failed") return "failed";
  return "completed";
}

function stringField(
  payload: Record<string, unknown>,
  key: string,
): string | undefined {
  const value = payload[key];
  return typeof value === "string" ? value : undefined;
}

function booleanField(
  payload: Record<string, unknown>,
  key: string,
): boolean | undefined {
  const value = payload[key];
  return typeof value === "boolean" ? value : undefined;
}

function numberField(
  payload: Record<string, unknown>,
  key: string,
): number | undefined {
  const value = payload[key];
  return typeof value === "number" ? value : undefined;
}

function objectField(
  payload: Record<string, unknown>,
  key: string,
): Record<string, unknown> | undefined {
  const value = payload[key];
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function taskOutcomeField(
  payload: Record<string, unknown>,
  key: string,
): "completed" | "partial_success" | "failed" | undefined {
  const value = stringField(payload, key);
  return value === "completed" ||
    value === "partial_success" ||
    value === "failed"
    ? value
    : undefined;
}

function policyDecisionField(
  payload: Record<string, unknown>,
  key: string,
): "allow" | "deny" | "prompt" | undefined {
  const value = stringField(payload, key);
  return value === "allow" || value === "deny" || value === "prompt"
    ? value
    : undefined;
}

function toolStatusField(
  payload: Record<string, unknown>,
  key: string,
): ToolCallStatus | "error" | undefined {
  const value = stringField(payload, key);
  return value === "queued" ||
    value === "running" ||
    value === "completed" ||
    value === "failed" ||
    value === "error"
    ? value
    : undefined;
}
