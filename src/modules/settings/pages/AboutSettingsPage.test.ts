import { strict as assert } from "node:assert";
import { describe, it } from "node:test";

import {
  APP_UPDATER_STATE_LABELS,
  updaterUiStateFromResult,
  type AppUpdaterUiState,
} from "./app-updater-state.ts";

describe("settings_ui — app updater state machine", () => {
  it("app_updater_state_machine_renders every APP-UPDATER-001 state", () => {
    const states: AppUpdaterUiState[] = [
      "idle",
      "checking",
      "available",
      "downloading",
      "ready",
      "failed",
    ];

    for (const state of states) {
      assert.ok(APP_UPDATER_STATE_LABELS[state].title);
      assert.ok(APP_UPDATER_STATE_LABELS[state].description);
    }
  });

  it("maps updater check results onto visible settings states", () => {
    assert.equal(updaterUiStateFromResult(null), "idle");
    assert.equal(
      updaterUiStateFromResult({
        status: "update_available",
        current_version: "0.4.0",
      }),
      "available",
    );
    assert.equal(
      updaterUiStateFromResult({
        status: "no_update",
        current_version: "0.4.0",
      }),
      "ready",
    );
    assert.equal(
      updaterUiStateFromResult({
        status: "failed",
        current_version: "0.4.0",
      }),
      "failed",
    );
  });
});
