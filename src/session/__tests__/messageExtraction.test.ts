// GF-03 PR-2 — unit tests for src/session/messageExtraction.ts.
//
// Mirrors the existing PR-1 idiom (node:test, source-text + behaviour
// hybrid).  Here we exercise the pure parsers directly because they
// have no React / Tauri dependencies.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  extractMemoryStoreFields,
  extractTodosFromToolResult,
} from "../messageExtraction.ts";

test("extractTodosFromToolResult: parses canonical `new_todos` payload", () => {
  const raw = JSON.stringify({
    new_todos: [
      { content: "Step A", status: "pending" },
      { content: "Step B", activeForm: "Doing B", status: "in_progress" },
      { content: "Step C", active_form: "Did C", status: "completed" },
    ],
  });
  const result = extractTodosFromToolResult(raw);
  assert.ok(result, "expected non-null TodoItem[]");
  assert.equal(result?.length, 3);
  // Falls back content → activeForm when missing.
  assert.equal(result?.[0]?.activeForm, "Step A");
  // camelCase activeForm wins over content default.
  assert.equal(result?.[1]?.activeForm, "Doing B");
  // snake_case `active_form` is honored too.
  assert.equal(result?.[2]?.activeForm, "Did C");
});

test("extractTodosFromToolResult: returns null on invalid / missing payload", () => {
  assert.equal(extractTodosFromToolResult(null), null);
  assert.equal(extractTodosFromToolResult(undefined), null);
  assert.equal(extractTodosFromToolResult(""), null);
  assert.equal(extractTodosFromToolResult("not json"), null);
  // Valid JSON but no recognized list key.
  assert.equal(extractTodosFromToolResult(JSON.stringify({ foo: 1 })), null);
  // Unrecognized status drops the entry, but still returns an array.
  const filtered = extractTodosFromToolResult(
    JSON.stringify({ new_todos: [{ content: "x", status: "weird" }] }),
  );
  assert.deepEqual(filtered, []);
});

test("extractMemoryStoreFields: lifts policy_decision / scope / reason_code", () => {
  const raw = JSON.stringify({
    policy_decision: "deny",
    scope: "project",
    reason_code: "filtered_pii",
    status: "blocked",
  });
  const out = extractMemoryStoreFields(raw);
  assert.deepEqual(out, {
    policyDecision: "deny",
    memoryScope: "project",
    memoryReasonCode: "filtered_pii",
  });
  // Invalid input → null (string, array, malformed JSON, missing fields).
  assert.equal(extractMemoryStoreFields(null), null);
  assert.equal(extractMemoryStoreFields("[]"), null);
  assert.equal(extractMemoryStoreFields("not json"), null);
  assert.equal(extractMemoryStoreFields(JSON.stringify({ unrelated: 1 })), null);
});
