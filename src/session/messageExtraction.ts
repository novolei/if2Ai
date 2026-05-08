// GF-03 PR-2 — pure parsers extracted from `App.tsx` (L609-718).
//
// These helpers run in two places: history replay (`loadConversationHistory`)
// and the live stream side-effect path (`sendMessage` → tool_result branch).
// Keeping them here lets both call sites share one canonical impl and lets
// us unit-test them without booting React.

import type { TodoItem } from "@/components/ui/TodoPanel";

/**
 * Coerce an arbitrary value (typically pulled from a tool result JSON
 * payload) into a strict `TodoItem`.  Returns `null` when required
 * fields are missing or the status enum doesn't match the canonical
 * three-value set so callers can `.filter(Boolean)` cleanly.
 */
export function normalizeTodoItem(value: unknown): TodoItem | null {
  if (!value || typeof value !== "object") return null;
  const item = value as Record<string, unknown>;
  const content = typeof item.content === "string" ? item.content : "";
  const activeForm =
    typeof item.activeForm === "string"
      ? item.activeForm
      : typeof item.active_form === "string"
        ? item.active_form
        : content;
  const status = item.status;
  if (
    !content ||
    (status !== "pending" &&
      status !== "in_progress" &&
      status !== "completed")
  ) {
    return null;
  }
  return {
    content,
    activeForm,
    status,
  };
}

/**
 * Extract `TodoItem[]` from a `TodoWrite` tool result payload.  Accepts
 * either `new_todos` (snake_case, backend canonical) or `newTodos`
 * (camelCase, legacy clients).  Returns `null` when the payload is
 * absent, not JSON, or contains no recognizable list — callers fall
 * back to keeping the previous todo state in that case.
 */
export function extractTodosFromToolResult(
  raw: string | null | undefined,
): TodoItem[] | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as {
      new_todos?: unknown[];
      newTodos?: unknown[];
    };
    const candidates = Array.isArray(parsed.new_todos)
      ? parsed.new_todos
      : Array.isArray(parsed.newTodos)
        ? parsed.newTodos
        : null;
    if (!candidates) return null;
    return candidates
      .map((item) => normalizeTodoItem(item))
      .filter((item): item is TodoItem => item !== null);
  } catch {
    return null;
  }
}

/**
 * Parse the structured `memory_store` tool result emitted by the backend
 * (`src-tauri/src/modules/tools/builtin/memory_store.rs`).  The handler
 * always returns a JSON object with `policy_decision`, `scope`,
 * `reason_code`, and `status`; we lift those onto `Message` so the
 * `MemoryStoreToolCard` highlight (deny / prompt) fires deterministically
 * instead of relying on the control-plane permission `policy_decision`,
 * which is unrelated to memory-write policy.
 *
 * Returns `null` when the result is missing, not JSON, or not produced
 * by the new structured `memory_store` handler — callers should fall
 * back to existing behaviour in that case so legacy tool flows are
 * unaffected.
 */
export function extractMemoryStoreFields(
  raw: string | null | undefined,
): {
  policyDecision?: "allow" | "deny" | "prompt";
  memoryScope?: "global" | "project" | "session";
  memoryReasonCode?: string;
} | null {
  if (!raw) return null;
  let parsed: Record<string, unknown>;
  try {
    const candidate = JSON.parse(raw);
    if (
      !candidate ||
      typeof candidate !== "object" ||
      Array.isArray(candidate)
    )
      return null;
    parsed = candidate as Record<string, unknown>;
  } catch {
    return null;
  }
  const decisionRaw = parsed.policy_decision;
  const scopeRaw = parsed.scope;
  const reasonCodeRaw = parsed.reason_code;
  const out: {
    policyDecision?: "allow" | "deny" | "prompt";
    memoryScope?: "global" | "project" | "session";
    memoryReasonCode?: string;
  } = {};
  if (
    decisionRaw === "allow" ||
    decisionRaw === "deny" ||
    decisionRaw === "prompt"
  ) {
    out.policyDecision = decisionRaw;
  }
  if (
    scopeRaw === "global" ||
    scopeRaw === "project" ||
    scopeRaw === "session"
  ) {
    out.memoryScope = scopeRaw;
  }
  if (typeof reasonCodeRaw === "string") {
    out.memoryReasonCode = reasonCodeRaw;
  }
  return Object.keys(out).length > 0 ? out : null;
}
