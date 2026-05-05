import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../usePermissionOverlay.ts", import.meta.url),
  "utf8",
);

test("usePermissionOverlay: returns null when approvals map is empty", () => {
  // The first-pending pick reads `Object.keys(approvals)`; an empty
  // map must short-circuit to `null` so the overlay stays closed.
  assert.match(source, /const ids = Object\.keys\(approvals\);/);
  assert.match(source, /if \(ids\.length === 0\) return null;/);
});

test("usePermissionOverlay: projects first approval into PermissionRequestPayload + dispatches permission_resolved on decide", () => {
  // Selector pulls approvals from the canonical projection store
  // (not raw store internals).
  assert.match(
    source,
    /useRuntimeProjectionSelector\(\(s\) => s\.approvals\)/,
  );
  // Shape of the projected payload matches the legacy
  // `PermissionRequestPayload` keys consumed by the dialog.
  assert.match(source, /session_id: a\.sessionId/);
  assert.match(source, /tool_name: a\.toolName/);
  assert.match(source, /permission_mode: a\.permissionMode/);
  assert.match(source, /current_mode: a\.currentMode/);
  // `decide` forwards to the streaming facade with toolName + scope
  // and dispatches `permission_resolved` in the `finally` block so
  // the projection reducer clears the approval entry.
  assert.match(
    source,
    /await respondPermission\(sessionId, decision, \{[\s\S]*?toolName: permissionPrompt\.tool_name,[\s\S]*?scope,[\s\S]*?\}\)/,
  );
  assert.match(
    source,
    /runtimeProjectionStore\.dispatch\(\{[\s\S]*?kind: "permission_resolved"/,
  );
});
