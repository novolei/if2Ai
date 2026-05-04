import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../useGlobalHotkeys.ts", import.meta.url),
  "utf8",
);

test("useGlobalHotkeys: Cmd+, opens settings window", () => {
  assert.match(source, /\(e\.metaKey \|\| e\.ctrlKey\) && e\.key === ","/);
  assert.match(source, /void openSettingsWindow\(\)/);
});

test("useGlobalHotkeys: Cmd+Shift+D toggles telemetry drawer", () => {
  // Capture-phase listener with shift+d code/key fallback.
  assert.match(source, /e\.metaKey \|\| e\.ctrlKey/);
  assert.match(source, /e\.shiftKey/);
  assert.match(source, /e\.code === "KeyD" \|\| e\.key\.toLowerCase\(\) === "d"/);
  assert.match(source, /setIsTelemetryDrawerOpen\(\(prev\) => !prev\)/);
  // Capture-phase registration is required so chat-ui inputs cannot
  // swallow the toggle before App-shell sees it.
  assert.match(source, /\{ capture: true \}/);
});

test("useGlobalHotkeys: returns drawer state + setter", () => {
  assert.match(source, /isTelemetryDrawerOpen,/);
  assert.match(source, /setTelemetryDrawerOpen: setIsTelemetryDrawerOpen,/);
});
