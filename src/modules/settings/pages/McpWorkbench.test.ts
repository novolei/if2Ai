import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const source = readFileSync(
  new URL("./McpWorkbenchPage.tsx", import.meta.url),
  "utf8",
);

describe("MCP Workbench UI contract", () => {
  it("renders the required workbench tabs", () => {
    for (const label of [
      "Servers",
      "Tools",
      "Resources",
      "Prompts",
      "Approvals",
      "Activity",
    ]) {
      assert.match(source, new RegExp(`\"${label}\"`));
    }
  });

  it("marks unsupported transports inactive and keeps activity visible", () => {
    assert.match(source, /server\.active \? "active" : "inactive"/);
    assert.match(source, /inactiveServers\.map/);
    assert.match(source, /entry\.status === "ok"/);
  });
});
