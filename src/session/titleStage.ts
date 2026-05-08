// GF-03 PR-4 — Pure session-title-stage helpers.
//
// Extracted from `src/App.tsx` (≈ L116-132 + L450-630).  All exports
// are pure — no React state, Tauri commands, or globals.  Behavior is
// pixel-identical to the original.  `formatSessionTitle`'s historical
// `activeConv?.title` closure fallback is preserved as an explicit
// `activeConvTitle` parameter (every in-tree call site already passes
// `conv`, but we keep the fallback to avoid behavioral drift).

import type {
  Conversation,
  Message,
  SessionTitleState,
} from "@/modules/chat/types";

export const PLACEHOLDER_SESSION_TITLE = "新对话";
export const MAX_AUTO_RENAME_COUNT = 1;
export const GENERIC_USER_PROMPTS = [
  "继续",
  "继续完成",
  "帮我看看",
  "看一下",
  "改一下",
  "优化一下",
  "修一下",
  "处理一下",
  "请继续",
  "开始",
  "你好",
  "hi",
  "hello",
];

/** Strip framing tokens / honorifics that don't carry topical meaning. */
export function normalizeSessionTitleSource(raw: string): string {
  const cleaned = raw
    .replace(/\[resume_cursor\][\s\S]*$/gi, "")
    .replace(
      /^(请|帮我|麻烦|继续|继续帮我|继续把|我想|想要|我要|需要|请先|先帮我)\s*/u,
      "",
    )
    .replace(/(一下|一下子|好吗|可以吗|吧|谢谢|thanks|thank you)\s*$/giu, "")
    .replace(/`+/g, "")
    .replace(/[#>*_\-\[\]]/g, " ")
    .replace(/\s+/g, " ")
    .trim();

  return cleaned;
}

/**
 * Distill a candidate title from a raw user message.  When `raw` is a
 * resume-cursor injection, fall back to the owning conversation's
 * title (so the displayed title doesn't churn mid-rename).
 */
export function formatSessionTitle(
  raw: string,
  conv?: Conversation,
  activeConvTitle?: string,
): string {
  if (raw.includes("[resume_cursor]")) {
    return conv?.title ?? activeConvTitle ?? "继续当前任务";
  }
  const cleaned = normalizeSessionTitleSource(raw);

  if (!cleaned) return PLACEHOLDER_SESSION_TITLE;
  const firstLine =
    cleaned
      .split(/[\n。！？!?]/)
      .find((segment) => segment.trim())
      ?.trim() ?? cleaned;
  return firstLine.slice(0, 30) || PLACEHOLDER_SESSION_TITLE;
}

function normalizeTitleComparison(value: string): string {
  return value.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
}

/** Two titles are similar if either contains the other's normalized form. */
export function areTitlesSimilar(a: string, b: string): boolean {
  const normalizedA = normalizeTitleComparison(a);
  const normalizedB = normalizeTitleComparison(b);
  if (!normalizedA || !normalizedB) return false;
  return (
    normalizedA === normalizedB ||
    normalizedA.includes(normalizedB) ||
    normalizedB.includes(normalizedA)
  );
}

/** True if the title is non-empty and not the placeholder ("新对话"). */
export function currentTitleIsResolved(title: string): boolean {
  const normalized = title.trim();
  return Boolean(normalized && normalized !== PLACEHOLDER_SESSION_TITLE);
}

/** True if a user message carries enough signal to seed a title. */
export function isMeaningfulUserMessage(content: string): boolean {
  const normalized = normalizeSessionTitleSource(content);
  if (!normalized) return false;
  if (normalized.includes("[resume_cursor]")) return false;
  if (normalized.length < 2) return false;
  const lower = normalized.toLowerCase();
  return !GENERIC_USER_PROMPTS.some((prompt) => lower === prompt);
}

/** Filter messages down to "meaningful" user turns. */
export function getMeaningfulUserMessages(messages: Message[]): Message[] {
  return messages.filter(
    (message) =>
      message.role === "user" && isMeaningfulUserMessage(message.content),
  );
}

/**
 * Pick the first user-message-derived title candidate, preferring a
 * message whose distillation is non-placeholder.
 */
export function getInitialSessionTitleCandidate(
  messages: Message[],
  conv?: Conversation,
  activeConvTitle?: string,
): string | null {
  const meaningfulMessages = getMeaningfulUserMessages(messages);
  const seedMessage =
    meaningfulMessages.find(
      (message) =>
        formatSessionTitle(message.content, conv, activeConvTitle) !==
        PLACEHOLDER_SESSION_TITLE,
    ) ?? meaningfulMessages[0];
  if (!seedMessage) return null;
  const nextTitle = formatSessionTitle(seedMessage.content, conv, activeConvTitle);
  return nextTitle === PLACEHOLDER_SESSION_TITLE ? null : nextTitle;
}

/**
 * If the latest meaningful user message reframes the topic away from
 * the current title, surface that as a correction candidate.
 */
export function getCorrectionTitleCandidate(
  messages: Message[],
  currentTitle: string,
  conv?: Conversation,
  activeConvTitle?: string,
): string | null {
  const userMessages = getMeaningfulUserMessages(messages);
  if (userMessages.length < 2) return null;
  const recentCandidates = userMessages
    .slice(-2)
    .map((message) => formatSessionTitle(message.content, conv, activeConvTitle))
    .filter(
      (candidate) => candidate && candidate !== PLACEHOLDER_SESSION_TITLE,
    );
  if (recentCandidates.length === 0) return null;
  const [previousCandidate, latestCandidate] = recentCandidates;
  const resolvedCandidate = latestCandidate ?? previousCandidate;
  if (!resolvedCandidate) return null;
  if (areTitlesSimilar(resolvedCandidate, currentTitle)) return null;
  return resolvedCandidate;
}

/**
 * Initial `SessionTitleState` derived from a session title.  Legacy
 * `_existingMessages` arg was always unused; kept for signature parity.
 */
export function getInitialSessionTitleState(
  title: string,
  _existingMessages: Message[] = [],
): SessionTitleState {
  if (title && title !== PLACEHOLDER_SESSION_TITLE) {
    return {
      stage: "locked",
      autoRenameCount: MAX_AUTO_RENAME_COUNT,
    };
  }
  return {
    stage: "placeholder",
    autoRenameCount: 0,
  };
}

/**
 * Build the user-facing "loop completion" status banner from the
 * scoped tool-message tally and an optional task outcome.
 */
export function buildLoopCompletionStatus(
  messages: Message[],
  streamId: string | undefined,
  taskOutcome?: Message["taskOutcome"],
  degradedReason?: string,
): { label: string; kind: NonNullable<Message["statusKind"]> } {
  const scopedToolMessages = messages.filter((msg) => {
    if (msg.role !== "tool") return false;
    if (!streamId) return true;
    return msg.streamId === streamId;
  });
  const total = scopedToolMessages.length;
  const completed = scopedToolMessages.filter(
    (msg) => msg.toolStatus === "completed",
  ).length;
  const failed = scopedToolMessages.filter(
    (msg) => msg.toolStatus === "error",
  ).length;

  if (taskOutcome === "partial_success") {
    if (degradedReason?.includes("max_iterations_reached")) {
      return {
        label:
          total > 0
            ? `本轮已完成 ${completed}/${total} 个步骤，达到迭代上限，可继续未完成部分`
            : "达到迭代上限，可继续未完成部分",
        kind: "partial",
      };
    }
    return {
      label:
        total > 0
          ? `本轮部分完成：已完成 ${completed}/${total} 个步骤`
          : "本轮任务部分完成，可继续补全",
      kind: "partial",
    };
  }

  if (taskOutcome === "failed") {
    return {
      label:
        total > 0
          ? `本轮执行失败：已完成 ${completed}/${total} 个步骤`
          : "本轮执行失败",
      kind: "failed",
    };
  }

  if (total === 0) return { label: "本轮执行完成", kind: "success" };
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
