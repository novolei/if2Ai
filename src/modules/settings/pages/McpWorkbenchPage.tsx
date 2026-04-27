import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  Activity,
  Boxes,
  FileText,
  KeyRound,
  Play,
  RefreshCw,
  Server,
  TerminalSquare,
} from "lucide-react";
import { toast } from "sonner";

import {
  mcpWorkbenchActivity,
  mcpWorkbenchCallTool,
  mcpWorkbenchDiscover,
  mcpWorkbenchGetPrompt,
  mcpWorkbenchListServers,
  mcpWorkbenchReadResource,
  type McpWorkbenchActivityEntry,
  type McpWorkbenchDiscovery,
  type McpWorkbenchPrompt,
  type McpWorkbenchResource,
  type McpWorkbenchServer,
  type McpWorkbenchTool,
} from "@/lib/tauri";
import { SettingsSurface } from "../components/SettingsSurface";

export const MCP_WORKBENCH_TABS = [
  "Servers",
  "Tools",
  "Resources",
  "Prompts",
  "Approvals",
  "Activity",
] as const;

type WorkbenchTab = (typeof MCP_WORKBENCH_TABS)[number];

function prettify(value: unknown): string {
  if (value == null) return "";
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function parseJsonObject(text: string): unknown {
  const trimmed = text.trim();
  if (!trimmed) return undefined;
  return JSON.parse(trimmed);
}

function statusClass(active: boolean): string {
  return active ? "bg-jade/10 text-jade" : "bg-black/[0.05] text-black/40";
}

function Section({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <SettingsSurface className="p-0">
      <div className="border-b border-black/[0.06] px-4 py-3">
        <h3 className="text-[13px] font-semibold text-black/75">{title}</h3>
      </div>
      <div className="p-4">{children}</div>
    </SettingsSurface>
  );
}

function JsonPreview({ value }: { value: unknown }) {
  if (value == null) return null;
  return (
    <pre className="max-h-64 overflow-auto rounded-lg border border-black/[0.06] bg-black/[0.025] p-3 text-[11px] text-black/60">
      {prettify(value)}
    </pre>
  );
}

export function McpWorkbenchPage() {
  const [tab, setTab] = useState<WorkbenchTab>("Servers");
  const [servers, setServers] = useState<McpWorkbenchServer[]>([]);
  const [discovery, setDiscovery] = useState<McpWorkbenchDiscovery>({
    tools: [],
    resources: [],
    prompts: [],
    unsupportedServers: [],
  });
  const [activity, setActivity] = useState<McpWorkbenchActivityEntry[]>([]);
  const [selectedTool, setSelectedTool] = useState("");
  const [toolArgs, setToolArgs] = useState("{\n  \"text\": \"hello\"\n}");
  const [toolResult, setToolResult] = useState<unknown>(null);
  const [selectedResource, setSelectedResource] =
    useState<McpWorkbenchResource | null>(null);
  const [resourceResult, setResourceResult] = useState<unknown>(null);
  const [selectedPrompt, setSelectedPrompt] =
    useState<McpWorkbenchPrompt | null>(null);
  const [promptArgs, setPromptArgs] = useState("{}");
  const [promptResult, setPromptResult] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const tools = discovery.tools;
  const resources = discovery.resources;
  const prompts = discovery.prompts;

  const inactiveServers = useMemo(
    () => servers.filter((server) => !server.active),
    [servers],
  );

  const refreshActivity = async () => {
    setActivity(await mcpWorkbenchActivity());
  };

  const refreshWorkbench = async () => {
    setBusy(true);
    setError(null);
    try {
      const [nextServers, nextDiscovery] = await Promise.all([
        mcpWorkbenchListServers(),
        mcpWorkbenchDiscover(),
      ]);
      setServers(nextServers);
      setDiscovery(nextDiscovery);
      setSelectedTool((current) => current || nextDiscovery.tools[0]?.qualifiedName || "");
      setSelectedResource((current) => current ?? nextDiscovery.resources[0] ?? null);
      setSelectedPrompt((current) => current ?? nextDiscovery.prompts[0] ?? null);
      await refreshActivity();
    } catch (err) {
      const message = String(err);
      setError(message);
      toast.error("MCP Workbench 刷新失败", { description: message });
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void refreshWorkbench();
  }, []);

  const callTool = async () => {
    if (!selectedTool) return;
    setBusy(true);
    setError(null);
    try {
      const result = await mcpWorkbenchCallTool({
        qualifiedToolName: selectedTool,
        arguments: parseJsonObject(toolArgs),
      });
      setToolResult(result.result);
      await refreshActivity();
    } catch (err) {
      const message = String(err);
      setError(message);
      toast.error("Tool 调用失败", { description: message });
    } finally {
      setBusy(false);
    }
  };

  const readResource = async () => {
    if (!selectedResource) return;
    setBusy(true);
    setError(null);
    try {
      const result = await mcpWorkbenchReadResource({
        serverName: selectedResource.serverName,
        uri: selectedResource.uri,
      });
      setResourceResult(result.result);
      await refreshActivity();
    } catch (err) {
      const message = String(err);
      setError(message);
      toast.error("Resource 读取失败", { description: message });
    } finally {
      setBusy(false);
    }
  };

  const getPrompt = async () => {
    if (!selectedPrompt) return;
    setBusy(true);
    setError(null);
    try {
      const result = await mcpWorkbenchGetPrompt({
        serverName: selectedPrompt.serverName,
        name: selectedPrompt.name,
        arguments: parseJsonObject(promptArgs),
      });
      setPromptResult(result.result);
      await refreshActivity();
    } catch (err) {
      const message = String(err);
      setError(message);
      toast.error("Prompt 获取失败", { description: message });
    } finally {
      setBusy(false);
    }
  };

  const selectedToolMeta = tools.find((tool) => tool.qualifiedName === selectedTool);

  return (
    <div className="space-y-4">
      <SettingsSurface className="px-4 py-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap gap-1 rounded-lg bg-black/[0.035] p-1">
            {MCP_WORKBENCH_TABS.map((item) => (
              <button
                key={item}
                type="button"
                onClick={() => setTab(item)}
                className={`h-8 rounded-md px-3 text-[12px] font-semibold ${
                  tab === item
                    ? "bg-white text-black/75 shadow-sm"
                    : "text-black/45 hover:text-black/65"
                }`}
              >
                {item}
              </button>
            ))}
          </div>
          <button
            type="button"
            disabled={busy}
            onClick={() => void refreshWorkbench()}
            className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-black/[0.08] px-3 text-[12px] font-semibold text-black/55 hover:bg-black/[0.03] disabled:opacity-50"
          >
            <RefreshCw className="h-3.5 w-3.5" />
            刷新
          </button>
        </div>
        {error && (
          <div className="mt-3 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-[12px] text-red-700">
            {error}
          </div>
        )}
      </SettingsSurface>

      {tab === "Servers" && (
        <Section title="Servers">
          <div className="divide-y divide-black/[0.06]">
            {servers.map((server) => (
              <div key={`${server.scope}:${server.name}`} className="flex items-center gap-3 py-2">
                <Server className="h-4 w-4 text-black/35" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-semibold text-black/75">
                      {server.name}
                    </span>
                    <span className={`rounded-full px-2 py-0.5 text-[10.5px] font-semibold ${statusClass(server.active)}`}>
                      {server.active ? "active" : "inactive"}
                    </span>
                  </div>
                  <p className="mt-0.5 truncate text-[11.5px] text-black/42">
                    {server.transport} · {server.scope}
                    {server.reason ? ` · ${server.reason}` : ""}
                  </p>
                </div>
              </div>
            ))}
          </div>
        </Section>
      )}

      {tab === "Tools" && (
        <Section title="Tools">
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(260px,360px)]">
            <div className="divide-y divide-black/[0.06]">
              {tools.map((tool: McpWorkbenchTool) => (
                <button
                  key={tool.qualifiedName}
                  type="button"
                  onClick={() => setSelectedTool(tool.qualifiedName)}
                  className="flex w-full items-start gap-3 py-2 text-left"
                >
                  <TerminalSquare className="mt-0.5 h-4 w-4 text-black/35" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13px] font-semibold text-black/75">
                      {tool.name}
                    </span>
                    <span className="block truncate text-[11.5px] text-black/42">
                      {tool.qualifiedName}
                    </span>
                  </span>
                </button>
              ))}
            </div>
            <div className="space-y-3">
              <textarea
                value={toolArgs}
                onChange={(event) => setToolArgs(event.target.value)}
                className="min-h-28 w-full rounded-lg border border-black/[0.08] bg-black/[0.02] p-3 font-mono text-[12px] outline-none focus:border-jade/35"
              />
              <button
                type="button"
                disabled={busy || !selectedTool}
                onClick={() => void callTool()}
                className="inline-flex h-8 items-center gap-1.5 rounded-lg bg-black px-3 text-[12px] font-semibold text-white disabled:opacity-50"
              >
                <Play className="h-3.5 w-3.5" />
                Test Tool
              </button>
              <JsonPreview value={selectedToolMeta?.inputSchema} />
              <JsonPreview value={toolResult} />
            </div>
          </div>
        </Section>
      )}

      {tab === "Resources" && (
        <Section title="Resources">
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(260px,420px)]">
            <div className="divide-y divide-black/[0.06]">
              {resources.map((resource) => (
                <button
                  key={`${resource.serverName}:${resource.uri}`}
                  type="button"
                  onClick={() => setSelectedResource(resource)}
                  className="flex w-full items-start gap-3 py-2 text-left"
                >
                  <FileText className="mt-0.5 h-4 w-4 text-black/35" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13px] font-semibold text-black/75">
                      {resource.name || resource.uri}
                    </span>
                    <span className="block truncate text-[11.5px] text-black/42">
                      {resource.serverName} · {resource.mimeType || "resource"}
                    </span>
                  </span>
                </button>
              ))}
            </div>
            <div className="space-y-3">
              <button
                type="button"
                disabled={busy || !selectedResource}
                onClick={() => void readResource()}
                className="inline-flex h-8 items-center gap-1.5 rounded-lg bg-black px-3 text-[12px] font-semibold text-white disabled:opacity-50"
              >
                <Play className="h-3.5 w-3.5" />
                Read Resource
              </button>
              <JsonPreview value={resourceResult} />
            </div>
          </div>
        </Section>
      )}

      {tab === "Prompts" && (
        <Section title="Prompts">
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(260px,420px)]">
            <div className="divide-y divide-black/[0.06]">
              {prompts.map((prompt) => (
                <button
                  key={`${prompt.serverName}:${prompt.name}`}
                  type="button"
                  onClick={() => setSelectedPrompt(prompt)}
                  className="flex w-full items-start gap-3 py-2 text-left"
                >
                  <Boxes className="mt-0.5 h-4 w-4 text-black/35" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13px] font-semibold text-black/75">
                      {prompt.name}
                    </span>
                    <span className="block truncate text-[11.5px] text-black/42">
                      {prompt.serverName} · {prompt.arguments.length} args
                    </span>
                  </span>
                </button>
              ))}
            </div>
            <div className="space-y-3">
              <textarea
                value={promptArgs}
                onChange={(event) => setPromptArgs(event.target.value)}
                className="min-h-24 w-full rounded-lg border border-black/[0.08] bg-black/[0.02] p-3 font-mono text-[12px] outline-none focus:border-jade/35"
              />
              <button
                type="button"
                disabled={busy || !selectedPrompt}
                onClick={() => void getPrompt()}
                className="inline-flex h-8 items-center gap-1.5 rounded-lg bg-black px-3 text-[12px] font-semibold text-white disabled:opacity-50"
              >
                <Play className="h-3.5 w-3.5" />
                Get Prompt
              </button>
              <JsonPreview value={promptResult} />
            </div>
          </div>
        </Section>
      )}

      {tab === "Approvals" && (
        <Section title="Approvals">
          <div className="space-y-2 text-[12px] text-black/55">
            <div className="flex items-center gap-2">
              <KeyRound className="h-4 w-4 text-black/35" />
              <span>Workbench tool calls use the same backend command boundary and activity ledger.</span>
            </div>
            {inactiveServers.map((server) => (
              <div key={`${server.scope}:${server.name}`} className="rounded-lg border border-black/[0.06] bg-black/[0.02] px-3 py-2">
                {server.name}: {server.reason}
              </div>
            ))}
          </div>
        </Section>
      )}

      {tab === "Activity" && (
        <Section title="Activity">
          <div className="divide-y divide-black/[0.06]">
            {activity.map((entry) => (
              <div key={`${entry.timestamp}:${entry.operation}:${entry.target ?? ""}`} className="py-2">
                <div className="flex items-center gap-2">
                  <Activity className="h-4 w-4 text-black/35" />
                  <span className="text-[12.5px] font-semibold text-black/75">
                    {entry.operation}
                  </span>
                  <span className={`rounded-full px-2 py-0.5 text-[10.5px] font-semibold ${entry.status === "ok" ? "bg-jade/10 text-jade" : "bg-red-50 text-red-600"}`}>
                    {entry.status}
                  </span>
                  <span className="text-[11px] text-black/35">{entry.durationMs}ms</span>
                </div>
                <p className="mt-1 truncate text-[11.5px] text-black/42">
                  {entry.serverId}{entry.target ? ` · ${entry.target}` : ""}
                  {entry.error ? ` · ${entry.error}` : ""}
                </p>
              </div>
            ))}
            {!activity.length && (
              <div className="py-8 text-center text-[12px] text-black/40">
                暂无 Workbench activity
              </div>
            )}
          </div>
        </Section>
      )}
    </div>
  );
}
