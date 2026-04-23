import { strict as assert } from "node:assert";
import { afterEach, describe, it } from "node:test";

import { resetApiClient, setApiClient, type ApiClient } from "@/api/client.ts";

import { refreshPendingPermission } from "./pending-permission-recovery.ts";
import { createRuntimeProjectionStore } from "./runtime-projection-store.ts";

afterEach(() => {
  resetApiClient();
});

describe("runtime projection — pending permission recovery", () => {
  it("fetches a pending permission and projects it into approvals", async () => {
    const client: ApiClient = {
      async call(command, args) {
        assert.equal(command, "get_pending_permission");
        assert.deepEqual(args, { sessionId: "session-42" });
        return {
          request_id: "permission-1",
          session_id: "session-42",
          tool_name: "bash",
          permission_mode: "dangerFullAccess",
          current_mode: "readOnly",
          message: "Tool 'bash' requires dangerFullAccess permission",
          requested_at: "2026-04-23T00:00:00Z",
        };
      },
      async subscribe() {
        return () => {};
      },
    };
    const store = createRuntimeProjectionStore({ flushMode: "sync" });
    setApiClient(client);

    const recovered = await refreshPendingPermission("session-42", store);

    assert.equal(recovered, true);
    const approval = store.getSnapshot().approvals["session-42"];
    assert.equal(typeof approval.receivedAt, "number");
    assert.deepEqual(approval, {
      sessionId: "session-42",
      toolName: "bash",
      permissionMode: "dangerFullAccess",
      currentMode: "readOnly",
      message: "Tool 'bash' requires dangerFullAccess permission",
      receivedAt: approval.receivedAt,
    });
  });

  it("does not change approvals when no pending permission exists", async () => {
    const client: ApiClient = {
      async call() {
        return null;
      },
      async subscribe() {
        return () => {};
      },
    };
    const store = createRuntimeProjectionStore({ flushMode: "sync" });
    setApiClient(client);

    const recovered = await refreshPendingPermission("session-empty", store);

    assert.equal(recovered, false);
    assert.deepEqual(store.getSnapshot().approvals, {});
  });
});
