// MIG-012 — frontend API facade transport adapter tests.
//
// Runs under Node's built-in `node --test` runner with the
// built-in TS stripping (Node >= 22.7). No test-runner
// dependency is added to package.json; the pack explicitly
// forbids introducing one.
//
// Invoke with:
//   node --test --experimental-strip-types \
//        src/api/client.test.ts src/api/streaming.test.ts ...
//
// Each test installs a mock [`ApiClient`] via `setApiClient`,
// exercises the relevant domain facade, and asserts that the
// facade forwarded the expected Tauri command name + args to
// the mock transport. If a future refactor swaps the Tauri
// wire command, these tests pin the contract.

import { strict as assert } from "node:assert";
import { afterEach, describe, it } from "node:test";

import {
  type ApiClient,
  type ApiEvent,
  getApiClient,
  resetApiClient,
  setApiClient,
} from "./client.ts";
import {
  startAgentStream,
  stopAgentStream,
  respondPermission,
} from "./streaming.ts";
import {
  createSession,
  deleteSession,
  getSession,
  renameSession,
  setSessionIdentity,
  setSessionPinned,
} from "./sessions.ts";
import {
  createProject,
  ensureDefaultWorkdir,
  listProjects,
  pickFolderDialog,
} from "./projects.ts";
import { executeSlashCommand, resolveSkillSlash } from "./slash.ts";
import { getOnboardingState } from "./onboarding.ts";
import { openSettingsWindow } from "./window.ts";

interface RecordedCall {
  command: string;
  args?: Record<string, unknown>;
}

function recordingClient(returns: Record<string, unknown> = {}): {
  client: ApiClient;
  calls: RecordedCall[];
  subscriptions: { event: string; count: number }[];
} {
  const calls: RecordedCall[] = [];
  const subscriptions: { event: string; count: number }[] = [];
  const client: ApiClient = {
    async call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
      calls.push({ command, args });
      return (returns[command] ?? undefined) as T;
    },
    async subscribe<T>(event: string, _handler: (e: ApiEvent<T>) => void) {
      const existing = subscriptions.find((s) => s.event === event);
      if (existing) {
        existing.count += 1;
      } else {
        subscriptions.push({ event, count: 1 });
      }
      return () => {};
    },
  };
  return { client, calls, subscriptions };
}

afterEach(() => {
  resetApiClient();
});

describe("api/client — transport abstraction", () => {
  it("getApiClient returns the default Tauri client before any override", () => {
    // Sanity: the default client is an object with both `call`
    // and `subscribe` methods — we do not invoke them here to
    // avoid hitting the real `@tauri-apps/api` module (which
    // would fail outside a Tauri window).
    const client = getApiClient();
    assert.equal(typeof client.call, "function");
    assert.equal(typeof client.subscribe, "function");
  });

  it("setApiClient installs the override and getApiClient returns it", () => {
    const { client } = recordingClient();
    setApiClient(client);
    assert.strictEqual(getApiClient(), client);
  });

  it("setApiClient(null) restores the default client", () => {
    const { client } = recordingClient();
    setApiClient(client);
    setApiClient(null);
    assert.notStrictEqual(getApiClient(), client);
  });

  it("resetApiClient restores the default client", () => {
    const { client } = recordingClient();
    setApiClient(client);
    resetApiClient();
    assert.notStrictEqual(getApiClient(), client);
  });
});

describe("api/streaming — wire contract", () => {
  it("startAgentStream forwards to `start_agent_stream` with camelCase args", async () => {
    const { client, calls } = recordingClient({
      start_agent_stream: "stream-id-123",
    });
    setApiClient(client);
    const id = await startAgentStream("session-7", "hello world", "readOnly");
    assert.equal(id, "stream-id-123");
    assert.deepEqual(calls, [
      {
        command: "start_agent_stream",
        args: {
          sessionId: "session-7",
          userMessage: "hello world",
          permissionMode: "readOnly",
        },
      },
    ]);
  });

  it("stopAgentStream forwards to `stop_agent_stream` with the stream id", async () => {
    const { client, calls } = recordingClient();
    setApiClient(client);
    await stopAgentStream("stream-xyz");
    assert.deepEqual(calls, [
      { command: "stop_agent_stream", args: { streamId: "stream-xyz" } },
    ]);
  });

  it("respondPermission forwards allow + scope + toolName", async () => {
    const { client, calls } = recordingClient();
    setApiClient(client);
    await respondPermission("session-42", "allow", {
      toolName: "bash",
      scope: "session",
    });
    assert.deepEqual(calls, [
      {
        command: "respond_permission",
        args: {
          sessionId: "session-42",
          decision: "allow",
          toolName: "bash",
          scope: "session",
        },
      },
    ]);
  });
});

describe("api/sessions — wire contract", () => {
  it("createSession forwards projectId + title", async () => {
    const { client, calls } = recordingClient({
      create_session: { id: "s1", title: "t" },
    });
    setApiClient(client);
    const meta = await createSession("project-1", "My session");
    assert.equal((meta as { id: string }).id, "s1");
    assert.deepEqual(calls, [
      {
        command: "create_session",
        args: { projectId: "project-1", title: "My session" },
      },
    ]);
  });

  it("createSession forwards optional identity override when provided", async () => {
    const { client, calls } = recordingClient({
      create_session: { id: "s1", title: "t" },
    });
    setApiClient(client);
    await createSession("project-1", "My session", {
      soul_id: "if2ai-core",
      persona_id: "staff-architect",
    });
    assert.deepEqual(calls, [
      {
        command: "create_session",
        args: {
          projectId: "project-1",
          title: "My session",
          identity: {
            soul_id: "if2ai-core",
            persona_id: "staff-architect",
          },
        },
      },
    ]);
  });

  it("getSession, renameSession, deleteSession, setSessionPinned, setSessionIdentity each map to their command", async () => {
    const { client, calls } = recordingClient({ get_session: { id: "s1" } });
    setApiClient(client);
    await getSession("s1");
    await renameSession("s1", "new-title");
    await deleteSession("s1");
    await setSessionPinned("s1", true);
    await setSessionIdentity("s1", {
      soul_id: "if2ai-core",
      persona_id: "execution-partner",
    });
    assert.deepEqual(
      calls.map((c) => c.command),
      [
        "get_session",
        "rename_session",
        "delete_session",
        "set_session_pinned",
        "set_session_identity",
      ],
    );
  });
});

describe("api/projects — wire contract", () => {
  it("createProject / listProjects / pickFolderDialog / ensureDefaultWorkdir dispatch correctly", async () => {
    const { client, calls } = recordingClient({
      list_projects: [],
      ensure_default_workdir: ["/tmp/w", "pid"],
    });
    setApiClient(client);
    await createProject("My app", "/tmp/foo");
    await listProjects();
    await pickFolderDialog();
    await ensureDefaultWorkdir();
    assert.deepEqual(
      calls.map((c) => c.command),
      [
        "create_project",
        "list_projects",
        "pick_folder_dialog",
        "ensure_default_workdir",
      ],
    );
    assert.deepEqual(calls[0].args, { name: "My app", workdir: "/tmp/foo" });
  });
});

describe("api/slash — wire contract", () => {
  it("executeSlashCommand + resolveSkillSlash forward args correctly", async () => {
    const { client, calls } = recordingClient({ resolve_skill_slash: null });
    setApiClient(client);
    await executeSlashCommand("/help", "session-1");
    await resolveSkillSlash("/my-skill run", "/home/user");
    assert.deepEqual(calls[0], {
      command: "execute_slash_command",
      args: { input: "/help", sessionId: "session-1" },
    });
    assert.deepEqual(calls[1], {
      command: "resolve_skill_slash",
      args: { input: "/my-skill run", cwd: "/home/user" },
    });
  });

  it("resolveSkillSlash passes `null` when cwd is omitted", async () => {
    const { client, calls } = recordingClient({ resolve_skill_slash: null });
    setApiClient(client);
    await resolveSkillSlash("/my-skill");
    assert.deepEqual(calls[0].args, { input: "/my-skill", cwd: null });
  });
});

describe("api/onboarding + api/window — wire contract", () => {
  it("getOnboardingState and openSettingsWindow dispatch their Tauri commands", async () => {
    const { client, calls } = recordingClient({ onboarding_get_state: {} });
    setApiClient(client);
    await getOnboardingState();
    await openSettingsWindow();
    assert.deepEqual(
      calls.map((c) => c.command),
      ["onboarding_get_state", "open_settings_window"],
    );
  });
});
