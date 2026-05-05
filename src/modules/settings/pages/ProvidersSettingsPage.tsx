/**
 * P-MULTI-API — Three-segment provider management page modelled after
 * openhanako-main's `ProvidersTab.tsx` (left list grouped by service
 * category, right detail panel with API Key / Base URL / API type
 * dropdown / model multi-select).
 *
 * Reuses the existing onboarding `provider_test` / `provider_list_models`
 * / `provider_configure_with_models` Tauri commands so this page is a
 * pure UI extension — no new IPC contracts.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, RefreshCw, Trash2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { fetchAvailableModelGroups } from "@/api/models";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { CompactInput } from "../components/CompactInput";
import { SettingsSurface } from "../components/SettingsSurface";
import { ThinkingModeChip } from "@/components/chat/ThinkingModeChip";
import {
  API_TYPE_OPTIONS,
  type ApiType,
  type Model,
  type ProviderConfig,
  type ServiceCategory,
} from "@/modules/onboarding/types";
import { useOnboarding } from "@/modules/onboarding/hooks/useOnboarding";
import { cn } from "@/lib/utils";

interface KnownProvider {
  id: string;
  display_name: string;
  default_base_url: string;
  default_api: ApiType;
  service_category: ServiceCategory;
  auth_type: "api-key" | "oauth" | "none";
  supports_models: boolean;
}

interface ThinkingProbeResult {
  modelId: string;
  supportsThinking: boolean;
  chunksRead: number;
  error?: string | null;
}

interface ModelCapabilitySelection {
  id: string;
  supportsThinking: boolean;
}

interface AvailableModelGroup {
  provider_id: string;
  provider_name: string;
  models: Array<{
    model_id: string;
    name: string;
    context_window?: number | null;
    reasoning?: boolean;
    reasoning_required_in_tool_calls?: boolean;
    supports_reasoning_effort?: boolean;
  }>;
}

/** Built-in provider catalogue mirrored from `known_providers.rs`.
 *  Ideally we expose this via a Tauri command; until then this small
 *  mirror keeps the Settings page self-contained.
 */
const KNOWN_PROVIDERS: KnownProvider[] = [
  // OAuth
  {
    id: "openai-codex-oauth",
    display_name: "ChatGPT Plus/Pro (Codex)",
    default_base_url: "",
    default_api: "openai-codex-responses",
    service_category: "oauth",
    auth_type: "oauth",
    supports_models: false,
  },
  // Coding Plan
  {
    id: "kimi-coding",
    display_name: "Kimi Coding Plan",
    default_base_url: "https://api.kimi.com/coding/",
    default_api: "anthropic-messages",
    service_category: "coding-plan",
    auth_type: "api-key",
    supports_models: true,
  },
  {
    id: "dashscope-coding",
    display_name: "百炼 Coding Plan",
    default_base_url: "https://coding.dashscope.aliyuncs.com/v1",
    default_api: "openai-completions",
    service_category: "coding-plan",
    auth_type: "api-key",
    supports_models: true,
  },
  {
    id: "volcengine-coding",
    display_name: "火山引擎 Coding Plan",
    default_base_url: "https://ark.cn-beijing.volces.com/api/coding/v3",
    default_api: "openai-completions",
    service_category: "coding-plan",
    auth_type: "api-key",
    supports_models: true,
  },
  // API
  ...[
    { id: "openai", display_name: "OpenAI", url: "https://api.openai.com/v1", api: "openai-completions" as ApiType },
    { id: "anthropic", display_name: "Anthropic", url: "https://api.anthropic.com", api: "anthropic-messages" as ApiType },
    { id: "deepseek", display_name: "DeepSeek", url: "https://api.deepseek.com/v1", api: "openai-completions" as ApiType },
    { id: "moonshot", display_name: "Moonshot (Kimi)", url: "https://api.moonshot.cn/v1", api: "openai-completions" as ApiType },
    { id: "ollama", display_name: "Ollama (本地)", url: "http://localhost:11434/v1", api: "openai-completions" as ApiType },
    { id: "openrouter", display_name: "OpenRouter", url: "https://openrouter.ai/api/v1", api: "openai-completions" as ApiType },
    { id: "gemini", display_name: "Google Gemini", url: "https://generativelanguage.googleapis.com/v1beta/openai", api: "openai-completions" as ApiType },
    { id: "minimax", display_name: "MiniMax", url: "https://api.minimaxi.com/anthropic", api: "anthropic-messages" as ApiType },
    { id: "dashscope", display_name: "阿里云百炼 (DashScope)", url: "https://dashscope.aliyuncs.com/compatible-mode/v1", api: "openai-completions" as ApiType },
    { id: "siliconflow", display_name: "SiliconFlow", url: "https://api.siliconflow.cn/v1", api: "openai-completions" as ApiType },
    { id: "zhipu", display_name: "智谱 AI (GLM)", url: "https://open.bigmodel.cn/api/paas/v4", api: "openai-completions" as ApiType },
    { id: "groq", display_name: "Groq", url: "https://api.groq.com/openai/v1", api: "openai-completions" as ApiType },
    { id: "mistral", display_name: "Mistral AI", url: "https://api.mistral.ai/v1", api: "openai-completions" as ApiType },
    { id: "xai", display_name: "xAI (Grok)", url: "https://api.x.ai/v1", api: "openai-completions" as ApiType },
  ].map<KnownProvider>((p) => ({
    id: p.id,
    display_name: p.display_name,
    default_base_url: p.url,
    default_api: p.api,
    service_category: "api",
    auth_type: p.id === "ollama" ? "none" : "api-key",
    supports_models: true,
  })),
];

const CATEGORY_ORDER: { key: ServiceCategory; label: string }[] = [
  { key: "oauth", label: "OAUTH" },
  { key: "coding-plan", label: "CODING PLAN" },
  { key: "api", label: "API" },
];

export function ProvidersSettingsPage() {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [configuredIds, setConfiguredIds] = useState<Set<string>>(new Set());
  const [modelCounts, setModelCounts] = useState<Map<string, number>>(new Map());
  const { getConfiguredProviders, getAllConfiguredModels } = useOnboarding();

  const refreshConfigured = useCallback(async () => {
    const [ids, modelMap] = await Promise.all([
      getConfiguredProviders(),
      getAllConfiguredModels(),
    ]);
    setConfiguredIds(new Set(ids));
    const counts = new Map<string, number>();
    modelMap.forEach((models, providerId) => counts.set(providerId, models.length));
    setModelCounts(counts);
    // Notify chat surfaces (HomeScreen / chat-ui) so the model
    // dropdown reflects the latest provider+model set without a
    // hard reload. Listened via `window.addEventListener('if2ai:models-changed', …)`.
    window.dispatchEvent(new CustomEvent("if2ai:models-changed"));
    void emit("if2ai://models-changed");
  }, [getConfiguredProviders, getAllConfiguredModels]);

  useEffect(() => {
    void refreshConfigured();
  }, [refreshConfigured]);

  const selected = useMemo(
    () => KNOWN_PROVIDERS.find((p) => p.id === selectedId) ?? null,
    [selectedId],
  );

  return (
    <SettingsSurface className="overflow-hidden p-0">
      <div className="grid h-[640px] grid-cols-[260px_1fr]">
        {/* ── 左栏：分组列表 ── */}
        <div className="overflow-y-auto border-r border-black/[0.06] bg-black/[0.012]">
          {CATEGORY_ORDER.map(({ key, label }) => {
            const items = KNOWN_PROVIDERS.filter((p) => p.service_category === key);
            if (items.length === 0) return null;
            return (
              <div key={key} className="py-1.5">
                <div className="px-3 py-1 text-[10px] font-semibold uppercase tracking-widest text-black/35">
                  {label}
                </div>
                {items.map((p) => {
                  const isConfigured = configuredIds.has(p.id);
                  const isSelected = selectedId === p.id;
                  const count = modelCounts.get(p.id) ?? 0;
                  return (
                    <button
                      key={p.id}
                      type="button"
                      onClick={() => setSelectedId(p.id)}
                      className={cn(
                        "flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left text-[12px] transition-colors hover:bg-black/[0.04]",
                        isSelected && "bg-jade/[0.08] text-jade",
                      )}
                    >
                      <div className="flex min-w-0 items-center gap-2">
                        <span
                          className={cn(
                            "h-1.5 w-1.5 shrink-0 rounded-full",
                            isConfigured ? "bg-jade" : "bg-black/15",
                          )}
                          aria-hidden
                        />
                        <span className="truncate">{p.display_name}</span>
                      </div>
                      <span className="shrink-0 text-[10.5px] text-black/35">{count}</span>
                    </button>
                  );
                })}
              </div>
            );
          })}
        </div>

        {/* ── 右栏：详情卡 ── */}
        <div className="overflow-y-auto px-6 py-5">
          {selected ? (
            <ProviderDetail
              provider={selected}
              isConfigured={configuredIds.has(selected.id)}
              onSaved={() => void refreshConfigured()}
            />
          ) : (
            <ProviderEmptyState />
          )}
        </div>
      </div>
    </SettingsSurface>
  );
}

function ProviderEmptyState() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 text-[12px] text-black/40">
      <span>从左侧选择一个服务商以配置 API Key、Base URL 与可用模型。</span>
      <span className="text-[10.5px] text-black/30">
        三个分组：OAuth（ChatGPT Plus/Pro）· Coding Plan（订阅制 SKU）· API（标准 Key 服务）
      </span>
    </div>
  );
}

interface ProviderDetailProps {
  provider: KnownProvider;
  isConfigured: boolean;
  onSaved: () => void;
}

function ProviderDetail({ provider, isConfigured, onSaved }: ProviderDetailProps) {
  const { testProvider, loadModels, getProviderConfig, getConfiguredModels } = useOnboarding();

  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState(provider.default_base_url);
  const [apiType, setApiType] = useState<ApiType>(provider.default_api);
  const [availableModels, setAvailableModels] = useState<Model[]>([]);
  const [selectedModelIds, setSelectedModelIds] = useState<Set<string>>(new Set());
  const [probeResults, setProbeResults] = useState<Map<string, ThinkingProbeResult>>(new Map());
  const [busy, setBusy] = useState(false);

  // Hydrate from saved config when the provider switches.
  useEffect(() => {
    setBaseUrl(provider.default_base_url);
    setApiType(provider.default_api);
    setApiKey("");
    setAvailableModels([]);
    setSelectedModelIds(new Set());
    setProbeResults(new Map());
    void (async () => {
      const [cfg, savedModelIds] = await Promise.all([
        getProviderConfig(provider.id),
        getConfiguredModels(provider.id),
      ]);
      if (cfg) {
        setApiKey(cfg.api_key ?? "");
        setBaseUrl(cfg.base_url ?? provider.default_base_url);
      }
      if (savedModelIds.length > 0) {
        let groupModels = new Map<string, AvailableModelGroup["models"][number]>();
        // ER-02 — facade-backed fetch replaces raw `invoke('model_list_available')`.
        const groups = (await fetchAvailableModelGroups()) as AvailableModelGroup[];
        const group = groups.find((item) => item.provider_id === provider.id);
        groupModels = new Map((group?.models ?? []).map((model) => [model.model_id, model]));
        setAvailableModels(
          savedModelIds.map((id) => {
            const saved = groupModels.get(id);
            return {
              id,
              name: saved?.name ?? id,
              context_window: saved?.context_window ?? null,
              max_tokens: null,
              modality: "Text",
              reasoning: saved?.reasoning ?? false,
              reasoning_required_in_tool_calls:
                saved?.reasoning_required_in_tool_calls ?? false,
              supports_reasoning_effort: saved?.supports_reasoning_effort ?? false,
            } satisfies Model;
          }),
        );
        setSelectedModelIds(new Set(savedModelIds));
      }
    })();
  }, [
    provider.id,
    provider.default_api,
    provider.default_base_url,
    getProviderConfig,
    getConfiguredModels,
  ]);

  const handleLoadModels = useCallback(async () => {
    setBusy(true);
    try {
      const models = await loadModels(
        provider.id,
        baseUrl || provider.default_base_url,
        apiKey || null,
      );
      setAvailableModels(models);
      if (models.length === 0) {
        toast.warning("未拉取到模型，请确认 Base URL / API Key 正确。");
      }
    } catch (e) {
      toast.error(`读取模型失败: ${(e as Error).message ?? e}`);
    } finally {
      setBusy(false);
    }
  }, [provider.id, provider.default_base_url, baseUrl, apiKey, loadModels]);

  const toggleModel = useCallback((id: string) => {
    setSelectedModelIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const probeSelectedModels = useCallback(
    async (modelIds: string[], config: ProviderConfig): Promise<ModelCapabilitySelection[]> => {
      if (modelIds.length === 0) return [];
      toast.info(`正在探测 ${modelIds.length} 个模型的 Thinking 支持...`);
      const results = await Promise.all(
        modelIds.map(async (modelId): Promise<ThinkingProbeResult> => {
          try {
            return await invoke<ThinkingProbeResult>("provider_probe_model_thinking", {
              providerConfig: config,
              modelId,
            });
          } catch (error) {
            return {
              modelId,
              supportsThinking: false,
              chunksRead: 0,
              error: (error as Error).message ?? String(error),
            };
          }
        }),
      );

      setProbeResults(new Map(results.map((result) => [result.modelId, result])));
      setAvailableModels((prev) =>
        prev.map((model) => {
          const probe = results.find((result) => result.modelId === model.id);
          if (!probe) return model;
          return {
            ...model,
            reasoning: probe.supportsThinking || model.reasoning,
            reasoning_required_in_tool_calls:
              probe.supportsThinking || model.reasoning_required_in_tool_calls,
          };
        }),
      );

      const supported = results.filter((result) => result.supportsThinking).length;
      const failed = results.filter((result) => result.error).length;
      if (failed > 0) {
        toast.warning(`Thinking 探测完成：${supported} 个支持，${failed} 个探测失败并按不支持写入。`);
      } else {
        toast.success(`Thinking 探测完成：${supported} 个模型支持。`);
      }

      return results.map((result) => ({
        id: result.modelId,
        supportsThinking: result.supportsThinking,
      }));
    },
    [],
  );

  const handleTest = useCallback(async () => {
    if (provider.auth_type === "api-key" && !apiKey) {
      toast.warning("请先填写 API Key。");
      return;
    }
    setBusy(true);
    try {
      const config: ProviderConfig = {
        provider_id: provider.id,
        api_key: apiKey || null,
        base_url: baseUrl || null,
        display_name: provider.display_name,
      };
      // testProvider throws on failure (Error.message carries reason);
      // resolves silently on success.
      await testProvider(config);
      toast.success("连接成功");
    } catch (e) {
      toast.error(`连接失败: ${(e as Error).message ?? e}`);
    } finally {
      setBusy(false);
    }
  }, [provider, apiKey, baseUrl, testProvider]);

  const handleSave = useCallback(async () => {
    if (selectedModelIds.size === 0 && availableModels.length > 0) {
      toast.warning("请至少选择一个模型。");
      return;
    }
    setBusy(true);
    try {
      const config: ProviderConfig = {
        provider_id: provider.id,
        api_key: apiKey || null,
        base_url: baseUrl || null,
        display_name: provider.display_name,
      };
      const selectedIds = Array.from(selectedModelIds);
      const models = await probeSelectedModels(selectedIds, config);
      await invoke("provider_configure_with_model_capabilities", {
        providerConfig: config,
        models,
      });
      toast.success("已保存，并写入模型 Thinking 探测结果");
      onSaved();
    } catch (e) {
      toast.error(`保存失败: ${(e as Error).message ?? e}`);
    } finally {
      setBusy(false);
    }
  }, [
    provider,
    apiKey,
    baseUrl,
    availableModels,
    selectedModelIds,
    probeSelectedModels,
    onSaved,
  ]);

  const handleDelete = useCallback(async () => {
    setBusy(true);
    try {
      await invoke("provider_delete", { providerId: provider.id });
      toast.success("已删除");
      onSaved();
    } catch (e) {
      // provider_delete may not exist in older builds — surface gracefully.
      toast.error(`删除失败: ${(e as Error).message ?? e}`);
    } finally {
      setBusy(false);
    }
  }, [provider.id, onSaved]);

  return (
    <div className="flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-[15px] font-medium">{provider.display_name}</h3>
          <p className="text-[11px] text-black/45">
            {provider.id} · {provider.service_category}
          </p>
        </div>
        {isConfigured ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="gap-1 rounded-xl text-rose-600"
            onClick={() => void handleDelete()}
          >
            <Trash2 className="h-3.5 w-3.5" />
            删除供应商
          </Button>
        ) : null}
      </div>

      {/* Credentials grid */}
      <div className="grid grid-cols-[80px_1fr] items-center gap-x-3 gap-y-2 text-[12px]">
        <label className="text-black/55">API Key</label>
        {provider.auth_type === "oauth" ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled
            className="justify-self-start rounded-md text-[11px]"
          >
            通过 OAuth 连接（即将上线）
          </Button>
        ) : (
          <CompactInput
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            type="password"
            autoComplete="off"
            spellCheck={false}
            placeholder={provider.auth_type === "none" ? "无需 API Key" : "sk-…"}
            disabled={provider.auth_type === "none"}
          />
        )}

        <label className="text-black/55">Base URL</label>
        <CompactInput
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          autoComplete="off"
          spellCheck={false}
          placeholder={provider.default_base_url}
        />

        <label className="text-black/55">API 类型</label>
        <select
          value={apiType}
          onChange={(e) => setApiType(e.target.value as ApiType)}
          className="rounded-md border border-black/10 bg-white px-2 py-1.5 text-[12px]"
        >
          {API_TYPE_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>

      {/* Selected models */}
      <div className="border-t border-black/[0.06] pt-3">
        <div className="mb-2 flex items-center justify-between">
          <div className="text-[12px] text-black/55">
            已添加的模型{" "}
            <span className="text-black/35">{selectedModelIds.size}</span>
          </div>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="gap-1 rounded-xl"
              disabled={busy || provider.auth_type === "oauth"}
              onClick={() => void handleLoadModels()}
            >
              <RefreshCw className={cn("h-3.5 w-3.5", busy && "animate-spin")} />
              读取模型
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="rounded-xl"
              disabled={busy}
              onClick={() => void handleTest()}
            >
              测试连接
            </Button>
          </div>
        </div>

        {availableModels.length === 0 ? (
          <p className="rounded-md border border-dashed border-black/10 bg-black/[0.02] px-3 py-4 text-center text-[11px] text-black/35">
            暂无已保存模型。点击「读取模型」从供应商加载可用模型。
          </p>
        ) : (
          <ul className="divide-y divide-black/[0.05] rounded-md border border-black/[0.06]">
            {availableModels.map((model) => {
              const checked = selectedModelIds.has(model.id);
              const probe = probeResults.get(model.id);
              return (
                <li key={model.id}>
                  <button
                    type="button"
                    onClick={() => toggleModel(model.id)}
                    className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-[12px] hover:bg-black/[0.02]"
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <span
                        className={cn(
                          "flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded border",
                          checked
                            ? "border-jade bg-jade text-white"
                            : "border-black/20 bg-white",
                        )}
                      >
                        {checked ? <Check className="h-2.5 w-2.5" /> : null}
                      </span>
                      <span className="truncate font-medium">{model.name}</span>
                      <span className="truncate text-[10.5px] text-black/35">
                        {model.id}
                      </span>
                    </div>
                    <div className="flex shrink-0 items-center gap-1.5">
                      <ThinkingModeChip model={model} compact />
                      {probe ? (
                        <span
                          className={cn(
                            "rounded px-1.5 text-[9.5px]",
                            probe.supportsThinking
                              ? "bg-jade/[0.10] text-jade"
                              : "bg-black/[0.05] text-black/35",
                          )}
                          title={probe.error || `chunks: ${probe.chunksRead}`}
                        >
                          {probe.supportsThinking ? "probe:on" : "probe:off"}
                        </span>
                      ) : null}
                      {model.context_window ? (
                        <span className="rounded bg-black/[0.05] px-1.5 text-[9.5px] text-black/40">
                          {(model.context_window / 1000).toFixed(0)}K
                        </span>
                      ) : null}
                    </div>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <div className="flex justify-end pt-2">
        <Button
          type="button"
          variant="default"
          size="sm"
          disabled={busy}
          onClick={() => void handleSave()}
        >
          保存
        </Button>
      </div>
    </div>
  );
}
