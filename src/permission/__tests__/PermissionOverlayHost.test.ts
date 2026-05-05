import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../PermissionOverlayHost.tsx", import.meta.url),
  "utf8",
);

test("PermissionOverlayHost: declared as component using shadcn Dialog from @/components/ui/dialog", () => {
  // Component declaration + Dialog import contract.
  assert.match(source, /export function PermissionOverlayHost/);
  assert.match(source, /from "@\/components\/ui\/dialog"/);
  assert.match(source, /<Dialog open=\{Boolean\(permissionPrompt\)\}>/);
  // Reads its data via the projection-backed hook (not raw store).
  assert.match(source, /usePermissionOverlay/);
});

test("PermissionOverlayHost: 4 decide buttons wire (decision, scope) tuples", () => {
  // Each of the 4 footer buttons must call `decide(...)` with the
  // exact (decision, scope) pair from the legacy App.tsx handler.
  assert.match(source, /onClick=\{\(\) => decide\("deny", "once"\)\}/);
  assert.match(source, /onClick=\{\(\) => decide\("deny", "session"\)\}/);
  assert.match(source, /onClick=\{\(\) => decide\("allow", "once"\)\}/);
  assert.match(source, /onClick=\{\(\) => decide\("allow", "session"\)\}/);
});
