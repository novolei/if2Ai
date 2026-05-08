import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(
  new URL("../useChatPrefill.ts", import.meta.url),
  "utf8",
);

test("useChatPrefill: subscribes to listenToChatPrefill on mount", () => {
  assert.match(source, /import \{ listenToChatPrefill \} from "@\/api"/);
  assert.match(source, /listenToChatPrefill\(\(payload\) => \{/);
});

test("useChatPrefill: forces chat section + sets input on non-empty prompt", () => {
  assert.match(source, /setActiveSection\("chat"\)/);
  assert.match(
    source,
    /typeof payload\?\.prompt === "string" && payload\.prompt\.trim\(\)/,
  );
  assert.match(source, /setInput\(payload\.prompt\)/);
});

test("useChatPrefill: cleans up listener on unmount", () => {
  // The cleanup branch must call the dispose returned by listenToChatPrefill.
  assert.match(source, /return \(\) => \{\s*if \(unlisten\) unlisten\(\);\s*\}/);
});
