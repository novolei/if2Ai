import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../useUpdaterBanner.ts", import.meta.url),
  "utf8",
);

test("useUpdaterBanner: boots by fetching state and subscribing", () => {
  // Initial fetch and live subscription wired on mount.
  assert.match(source, /void getAppUpdaterState\(\)/);
  assert.match(source, /void onAppUpdaterState\(/);
  // Cleanup paths cancel + unlisten.
  assert.match(source, /unlisten\?\.\(\)/);
});

test("useUpdaterBanner: throttles background check to 6h", () => {
  // The constant must mirror the original App.tsx literal.
  assert.match(source, /const SIX_HOURS_MS = 6 \* 60 \* 60 \* 1000;/);
  // The early-return guard against `now - lastCheck < SIX_HOURS_MS`.
  assert.match(source, /if \(now - lastCheck < SIX_HOURS_MS\) return;/);
  // localStorage key for the last-check timestamp.
  assert.match(source, /"if2ai:app-updater:last-check-ms"/);
});

test("useUpdaterBanner: dismiss persists banner version to localStorage", () => {
  // Dismiss writes the version to the canonical key and updates state.
  assert.match(source, /"if2ai:app-updater:dismissed-banner-version"/);
  assert.match(
    source,
    /localStorage\.setItem\(DISMISSED_BANNER_KEY, version\)/,
  );
  // Banner visibility comparison ignores already-dismissed versions.
  assert.match(
    source,
    /dismissedBannerVersion !== latestUpdaterVersion/,
  );
});

test("useUpdaterBanner: runUpdater short-circuits busy states", () => {
  // Mirrors the original guard preventing concurrent flows.
  assert.match(
    source,
    /status === "checking"[\s\S]*status === "downloading"[\s\S]*status === "installing"/,
  );
  // Full flow ends in downloadAndInstallAppUpdate when available.
  assert.match(source, /downloadAndInstallAppUpdate\(\)/);
});
