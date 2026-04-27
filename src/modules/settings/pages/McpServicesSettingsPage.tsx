import { useEffect, useMemo, useState } from "react";
import {
  Pencil,
  Plus,
  RefreshCw,
  Save,
  Server,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";

import {
  getMcpServiceConfig,
  setMcpServiceConfig,
  type McpServiceConfig,
  type McpServiceEntry,
  type McpServiceEntryInput,
  type McpServiceTransport,
} from "@/lib/tauri";
import { SettingsSurface } from "../components/SettingsSurface";
import { CompactInput } from "../components/CompactInput";
import { McpWorkbenchPage } from "./McpWorkbenchPage";

const TRANSPORTS: McpServiceTransport[] = [
  "stdio",
  "http",
  "sse",
  "ws",
  "sdk",
  "claudeai-proxy",
];

const emptyDraft: McpServiceEntryInput = {
  name: "",
  transport: "stdio",
  command: "",
  args: [],
  env: {},
  url: "",
  headers: {},
  headers_helper: "",
  sdk_name: "",
  proxy_id: "",
};

function linesToMap(value: string): Record<string, string> {
  return Object.fromEntries(
    value
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => {
        const index = line.indexOf("=");
        return index === -1
          ? [line, ""]
          : [line.slice(0, index).trim(), line.slice(index + 1).trim()];
      }),
  );
}

function mapToLines(value: Record<string, string>): string {
  return Object.entries(value)
    .map(([key, item]) => `${key}=${item}`)
    .join("\n");
}

function toInput(entry: McpServiceEntry): McpServiceEntryInput {
  return {
    name: entry.name,
    transport: entry.transport,
    command: entry.command ?? "",
    args: entry.args ?? [],
    env: entry.env ?? {},
    url: entry.url ?? "",
    headers: entry.headers ?? {},
    headers_helper: entry.headers_helper ?? "",
    sdk_name: entry.sdk_name ?? "",
    proxy_id: entry.proxy_id ?? "",
  };
}

function compactSummary(entry: McpServiceEntry): string {
  if (entry.transport === "stdio") {
    return [entry.command, ...(entry.args ?? [])].filter(Boolean).join(" ");
  }
  if (entry.transport === "sdk") return entry.sdk_name ?? "";
  if (entry.transport === "claudeai-proxy") {
    return [entry.proxy_id, entry.url].filter(Boolean).join(" · ");
  }
  return entry.url ?? "";
}

export function McpServicesSettingsPage() {
  const [activePanel, setActivePanel] = useState<"services" | "workbench">("services");
  const [config, setConfig] = useState<McpServiceConfig | null>(null);
  const [draft, setDraft] = useState<McpServiceEntryInput>(emptyDraft);
  const [argsText, setArgsText] = useState("");
  const [envText, setEnvText] = useState("");
  const [headersText, setHeadersText] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const userServers = useMemo(
    () => config?.servers.filter((server) => server.scope === "user").map(toInput) ?? [],
    [config],
  );
  const stdioCount =
    config?.servers.filter((server) => server.manager_supported).length ?? 0;

  const refresh = async () => {
    setLoading(true);
    setError(null);
    try {
      setConfig(await getMcpServiceConfig());
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const editEntry = (entry: McpServiceEntry) => {
    const next = toInput(entry);
    setDraft(next);
    setArgsText((next.args ?? []).join("\n"));
    setEnvText(mapToLines(next.env ?? {}));
    setHeadersText(mapToLines(next.headers ?? {}));
  };

  const resetDraft = () => {
    setDraft(emptyDraft);
    setArgsText("");
    setEnvText("");
    setHeadersText("");
  };

  const normalizedDraft = (): McpServiceEntryInput => ({
    ...draft,
    name: draft.name.trim(),
    command: draft.command?.trim() ?? "",
    url: draft.url?.trim() ?? "",
    sdk_name: draft.sdk_name?.trim() ?? "",
    proxy_id: draft.proxy_id?.trim() ?? "",
    headers_helper: draft.headers_helper?.trim() ?? "",
    args: argsText
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean),
    env: linesToMap(envText),
    headers: linesToMap(headersText),
  });

  const saveServers = async (servers: McpServiceEntryInput[]) => {
    setSaving(true);
    setError(null);
    try {
      const saved = await setMcpServiceConfig({ servers });
      setConfig(saved);
      toast.success("MCP 服务配置已保存");
    } catch (err) {
      setError(String(err));
      toast.error("保存失败", { description: String(err) });
    } finally {
      setSaving(false);
    }
  };

  const upsertDraft = async () => {
    const next = normalizedDraft();
    if (!next.name) {
      setError("MCP 服务名不能为空");
      return;
    }
    const others = userServers.filter((server) => server.name !== next.name);
    await saveServers([...others, next]);
    resetDraft();
  };

  const removeUserServer = async (name: string) => {
    await saveServers(userServers.filter((server) => server.name !== name));
  };

  return (
    <div className="space-y-4 p-5">
      <div className="flex w-fit gap-1 rounded-lg bg-black/[0.035] p-1">
        {[
          ["services", "服务配置"],
          ["workbench", "Workbench"],
        ].map(([key, label]) => (
          <button
            key={key}
            type="button"
            onClick={() => setActivePanel(key as "services" | "workbench")}
            className={`h-8 rounded-md px-3 text-[12px] font-semibold ${
              activePanel === key
                ? "bg-white text-black/75 shadow-sm"
                : "text-black/45 hover:text-black/65"
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {activePanel === "workbench" ? (
        <McpWorkbenchPage />
      ) : (
        <>
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Server className="h-4 w-4 text-black/55" />
              <h2 className="text-[15px] font-semibold text-black/80">
                MCP 服务
              </h2>
            </div>
            <p className="mt-1 truncate text-[12px] text-black/45">
              用户配置写入 {config?.user_settings_path ?? "~/.if2ai/mcp/settings.json"}
            </p>
          </div>
          <button
            type="button"
            onClick={() => void refresh()}
            disabled={loading}
            className="inline-flex h-8 items-center gap-1.5 rounded-xl border border-black/[0.08] px-3 text-[12px] font-medium text-black/60 hover:bg-black/[0.03] disabled:opacity-50"
          >
            <RefreshCw className="h-3.5 w-3.5" />
            刷新
          </button>
        </div>

        <div className="mt-4 grid grid-cols-3 gap-2">
          {[
            ["总数", config?.servers.length ?? 0],
            ["stdio 可运行", stdioCount],
            ["用户层", userServers.length],
          ].map(([label, value]) => (
            <div
              key={label}
              className="rounded-xl border border-black/[0.06] bg-black/[0.015] px-3 py-2"
            >
              <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/35">
                {label}
              </div>
              <div className="mt-0.5 text-[16px] font-semibold text-black/75">
                {value}
              </div>
            </div>
          ))}
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-0">
        <div className="border-b border-black/[0.06] px-5 py-3.5">
          <h3 className="text-[13px] font-semibold text-black/75">
            生效服务
          </h3>
        </div>
        <div className="divide-y divide-black/[0.06]">
          {config?.servers.length ? (
            config.servers.map((server) => (
              <div
                key={`${server.scope}:${server.name}`}
                className="flex items-center gap-3 px-5 py-3"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate text-[13px] font-semibold text-black/75">
                      {server.name}
                    </span>
                    <span className="rounded-full bg-black/[0.04] px-2 py-0.5 text-[10.5px] font-semibold text-black/50">
                      {server.transport}
                    </span>
                    <span className="rounded-full bg-jade/10 px-2 py-0.5 text-[10.5px] font-semibold text-jade">
                      {server.scope}
                    </span>
                  </div>
                  <p className="mt-1 truncate font-mono text-[11.5px] text-black/45">
                    {compactSummary(server) || "未配置入口"}
                  </p>
                </div>
                <span className="hidden shrink-0 text-[11px] text-black/35 sm:inline">
                  {server.manager_supported ? "stdio manager" : "parsed only"}
                </span>
                <button
                  type="button"
                  onClick={() => editEntry(server)}
                  className="inline-flex h-7 w-7 items-center justify-center rounded-lg text-black/45 hover:bg-black/[0.04]"
                  title="编辑"
                >
                  <Pencil className="h-3.5 w-3.5" />
                </button>
                {server.scope === "user" && (
                  <button
                    type="button"
                    onClick={() => void removeUserServer(server.name)}
                    className="inline-flex h-7 w-7 items-center justify-center rounded-lg text-red-500/70 hover:bg-red-50"
                    title="删除"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                )}
              </div>
            ))
          ) : (
            <div className="px-5 py-8 text-center text-[12px] text-black/40">
              尚未配置 MCP 服务
            </div>
          )}
        </div>
      </SettingsSurface>

      <SettingsSurface className="px-5 py-4">
        <div className="grid gap-3 md:grid-cols-[1fr_150px]">
          <CompactInput
            label="服务名"
            value={draft.name}
            onChange={(event) =>
              setDraft((current) => ({ ...current, name: event.target.value }))
            }
            placeholder="browser-use"
          />
          <label className="flex flex-col gap-1.5">
            <span className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
              传输
            </span>
            <select
              value={draft.transport}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  transport: event.target.value as McpServiceTransport,
                }))
              }
              className="h-8 rounded-xl border border-black/[0.09] bg-black/[0.02] px-3 text-[12px] font-medium outline-none focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
            >
              {TRANSPORTS.map((transport) => (
                <option key={transport} value={transport}>
                  {transport}
                </option>
              ))}
            </select>
          </label>
        </div>

        {draft.transport === "stdio" ? (
          <div className="mt-3 grid gap-3 md:grid-cols-2">
            <CompactInput
              label="Command"
              value={draft.command ?? ""}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  command: event.target.value,
                }))
              }
              placeholder="uvx"
            />
            <textarea
              value={argsText}
              onChange={(event) => setArgsText(event.target.value)}
              placeholder={"每行一个参数\nbrowser-use-mcp"}
              className="min-h-20 rounded-xl border border-black/[0.09] bg-black/[0.02] px-3 py-2 font-mono text-[12px] outline-none focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
            />
          </div>
        ) : (
          <div className="mt-3 grid gap-3 md:grid-cols-2">
            <CompactInput
              label={draft.transport === "sdk" ? "SDK Name" : "URL"}
              value={draft.transport === "sdk" ? draft.sdk_name ?? "" : draft.url ?? ""}
              onChange={(event) =>
                setDraft((current) =>
                  draft.transport === "sdk"
                    ? { ...current, sdk_name: event.target.value }
                    : { ...current, url: event.target.value },
                )
              }
              placeholder={draft.transport === "sdk" ? "builtin-name" : "https://..."}
            />
            {draft.transport === "claudeai-proxy" && (
              <CompactInput
                label="Proxy ID"
                value={draft.proxy_id ?? ""}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    proxy_id: event.target.value,
                  }))
                }
              />
            )}
          </div>
        )}

        <div className="mt-3 grid gap-3 md:grid-cols-2">
          <textarea
            value={envText}
            onChange={(event) => setEnvText(event.target.value)}
            placeholder="ENV_KEY=value"
            className="min-h-20 rounded-xl border border-black/[0.09] bg-black/[0.02] px-3 py-2 font-mono text-[12px] outline-none focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
          />
          <textarea
            value={headersText}
            onChange={(event) => setHeadersText(event.target.value)}
            placeholder="Header=value"
            className="min-h-20 rounded-xl border border-black/[0.09] bg-black/[0.02] px-3 py-2 font-mono text-[12px] outline-none focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
          />
        </div>

        <div className="mt-4 flex items-center justify-between gap-3">
          <p className="truncate text-[12px] text-black/40">
            project/local 服务只读；编辑它会保存为用户层覆盖。
          </p>
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              onClick={resetDraft}
              className="h-8 rounded-xl px-3 text-[12px] font-medium text-black/45 hover:bg-black/[0.03]"
            >
              清空
            </button>
            <button
              type="button"
              disabled={saving}
              onClick={() => void upsertDraft()}
              className="inline-flex h-8 items-center gap-1.5 rounded-xl bg-black px-3 text-[12px] font-semibold text-white disabled:opacity-50"
            >
              {draft.name ? <Save className="h-3.5 w-3.5" /> : <Plus className="h-3.5 w-3.5" />}
              保存服务
            </button>
          </div>
        </div>
        {error && (
          <div className="mt-3 rounded-xl border border-red-200 bg-red-50 px-3 py-2 text-[12px] text-red-700">
            {error}
          </div>
        )}
      </SettingsSurface>
        </>
      )}
    </div>
  );
}
