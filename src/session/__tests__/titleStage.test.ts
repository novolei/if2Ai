// GF-03 PR-4 — unit tests for src/session/titleStage.ts.
//
// All helpers under test are pure (no React / Tauri).  We exercise
// boundary cases for each exported function so behavior changes are
// caught immediately by `npm test`.

import assert from "node:assert/strict";
import { test } from "node:test";

import type { Conversation, Message } from "../../modules/chat/types.ts";
import {
  GENERIC_USER_PROMPTS,
  MAX_AUTO_RENAME_COUNT,
  PLACEHOLDER_SESSION_TITLE,
  areTitlesSimilar,
  buildLoopCompletionStatus,
  currentTitleIsResolved,
  formatSessionTitle,
  getCorrectionTitleCandidate,
  getInitialSessionTitleCandidate,
  getInitialSessionTitleState,
  getMeaningfulUserMessages,
  isMeaningfulUserMessage,
  normalizeSessionTitleSource,
} from "../titleStage.ts";

const ts = new Date("2026-01-01T00:00:00Z");

const userMessage = (content: string): Message => ({
  id: `u-${content}`,
  role: "user",
  content,
  timestamp: ts,
});
const assistantMessage = (content: string): Message => ({
  id: `a-${content}`,
  role: "assistant",
  content,
  timestamp: ts,
});

test("formatSessionTitle: empty input → placeholder", () => {
  assert.equal(formatSessionTitle(""), PLACEHOLDER_SESSION_TITLE);
  assert.equal(formatSessionTitle("   "), PLACEHOLDER_SESSION_TITLE);
});

test("formatSessionTitle: short prompt is preserved", () => {
  assert.equal(formatSessionTitle("帮我写个排序算法"), "写个排序算法");
});

test("formatSessionTitle: long prompt is truncated to 30 chars and split on punctuation", () => {
  const long = "实现一个分布式缓存系统，需要支持LRU淘汰、读写穿透、热点探测、自动扩缩容、跨机房复制和多租户隔离。";
  const result = formatSessionTitle(long);
  assert.ok(result.length <= 30, `expected ≤ 30 chars, got ${result.length}`);
  assert.notEqual(result, PLACEHOLDER_SESSION_TITLE);
});

test("formatSessionTitle: resume_cursor falls back to conv title then activeConvTitle", () => {
  const conv: Conversation = {
    id: "s1",
    projectId: "p1",
    title: "Conv Title",
    titleIcon: null,
    messages: [],
    updatedAt: ts,
  };
  assert.equal(formatSessionTitle("[resume_cursor] xyz", conv), "Conv Title");
  assert.equal(
    formatSessionTitle("[resume_cursor]", undefined, "Active Title"),
    "Active Title",
  );
  assert.equal(formatSessionTitle("[resume_cursor]"), "继续当前任务");
});

test("getInitialSessionTitleCandidate: only generic prompts → null", () => {
  // Use prompts with no honorific prefix so normalization preserves
  // them and the GENERIC_USER_PROMPTS exact-match guard fires.
  const messages = ["hi", "hello", "你好"].map(userMessage);
  assert.equal(getInitialSessionTitleCandidate(messages), null);
});

test("GENERIC_USER_PROMPTS array is non-empty (sanity)", () => {
  assert.ok(GENERIC_USER_PROMPTS.length >= 5);
});

test("getInitialSessionTitleCandidate: meaningful first user message becomes candidate", () => {
  const messages = [userMessage("继续"), userMessage("帮我重构状态机")];
  const candidate = getInitialSessionTitleCandidate(messages);
  assert.equal(candidate, "重构状态机");
});

test("getInitialSessionTitleState: placeholder vs explicit", () => {
  assert.deepEqual(getInitialSessionTitleState(PLACEHOLDER_SESSION_TITLE), {
    stage: "placeholder",
    autoRenameCount: 0,
  });
  assert.deepEqual(getInitialSessionTitleState("Real Title"), {
    stage: "locked",
    autoRenameCount: MAX_AUTO_RENAME_COUNT,
  });
  assert.deepEqual(getInitialSessionTitleState(""), {
    stage: "placeholder",
    autoRenameCount: 0,
  });
});

test("areTitlesSimilar: exact / case-insensitive / containment / disjoint", () => {
  assert.equal(areTitlesSimilar("hello", "hello"), true);
  assert.equal(areTitlesSimilar("Hello World", "hello world"), true);
  assert.equal(areTitlesSimilar("缓存系统", "分布式缓存系统设计"), true);
  assert.equal(areTitlesSimilar("foo", "bar"), false);
  assert.equal(areTitlesSimilar("", "hello"), false);
});

test("currentTitleIsResolved: placeholder / blank are not resolved", () => {
  assert.equal(currentTitleIsResolved(""), false);
  assert.equal(currentTitleIsResolved("   "), false);
  assert.equal(currentTitleIsResolved(PLACEHOLDER_SESSION_TITLE), false);
  assert.equal(currentTitleIsResolved("Real Title"), true);
});

test("isMeaningfulUserMessage / getMeaningfulUserMessages", () => {
  assert.equal(isMeaningfulUserMessage(""), false);
  assert.equal(isMeaningfulUserMessage("hi"), false);
  assert.equal(isMeaningfulUserMessage("[resume_cursor] something"), false);
  assert.equal(isMeaningfulUserMessage("帮我重构状态机"), true);
  const messages = [
    userMessage("hi"),
    userMessage("帮我重构状态机"),
    assistantMessage("ok"),
    userMessage("继续"),
  ];
  const meaningful = getMeaningfulUserMessages(messages);
  assert.equal(meaningful.length, 1);
  assert.equal(meaningful[0]?.content, "帮我重构状态机");
});

test("normalizeSessionTitleSource: strips leading honorific + trailing tail", () => {
  assert.equal(normalizeSessionTitleSource("帮我写个排序算法 谢谢"), "写个排序算法");
  assert.equal(normalizeSessionTitleSource("```code```"), "code");
  // Resume cursor token is removed.
  assert.equal(
    normalizeSessionTitleSource("帮我看 [resume_cursor] xyz"),
    "看",
  );
});

test("getCorrectionTitleCandidate: returns latest candidate when distinct from current title", () => {
  const messages = [userMessage("帮我重构状态机"), userMessage("帮我写个排序算法")];
  const candidate = getCorrectionTitleCandidate(messages, "重构状态机");
  assert.equal(candidate, "写个排序算法");
});

test("getCorrectionTitleCandidate: similar title → null", () => {
  const messages = [userMessage("帮我重构状态机"), userMessage("帮我重构状态机的实现")];
  const candidate = getCorrectionTitleCandidate(messages, "重构状态机");
  assert.equal(candidate, null);
});

test("buildLoopCompletionStatus: empty tools → 本轮执行完成", () => {
  const result = buildLoopCompletionStatus([], "stream-1");
  assert.deepEqual(result, { label: "本轮执行完成", kind: "success" });
});

test("buildLoopCompletionStatus: failure / partial outcomes labelled appropriately", () => {
  const tools: Message[] = [
    {
      id: "t1",
      role: "tool",
      content: "",
      timestamp: ts,
      streamId: "s",
      toolStatus: "completed",
    },
    {
      id: "t2",
      role: "tool",
      content: "",
      timestamp: ts,
      streamId: "s",
      toolStatus: "error",
    },
  ];
  const failed = buildLoopCompletionStatus(tools, "s", "failed");
  assert.equal(failed.kind, "failed");
  const partial = buildLoopCompletionStatus(
    tools,
    "s",
    "partial_success",
    "max_iterations_reached",
  );
  assert.equal(partial.kind, "partial");
  assert.match(partial.label, /迭代上限/);
});
