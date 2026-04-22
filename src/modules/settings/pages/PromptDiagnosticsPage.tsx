import { useEffect, useMemo, useState, type ReactNode } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { SessionIdentityEditor } from "@/components/identity/SessionIdentityEditor";
import {
  getIdentityPack,
  updateSessionIdentity,
  type IdentityCustomizationPack,
} from "@/api/identity";
import { getSession } from "@/api/sessions";
import { broadcastChange, useCrossWindowChange } from "@/lib/crossWindowSync";
import {
  getPromptControlCatalog,
  getPromptControlSettings,
  setPromptControlSettings,
  type PromptControlCatalog,
  type PromptControlSettings,
  type PromptScenarioProfile,
  type Session,
} from "@/lib/tauri";
import { PromptDiagnosticsPanel } from "@/modules/prompt-diagnostics/components/PromptDiagnosticsPanel";
import { SettingsSurface } from "../components/SettingsSurface";
import {
  readLatestPromptDiagnosticsSnapshot,
  type PromptDiagnosticsSnapshot,
} from "@/modules/prompt-diagnostics/storage";
import { useRuntimeProjectionSelector } from "@/runtime-projection/use-runtime-projection";

const PROFILE_OPTIONS: Array<{
  value: PromptScenarioProfile | "auto";
  label: string;
  description: string;
}> = [
  {
    value: "auto",
    label: "Auto",
    description: "优先让 request intelligence 决定当前 turn 的场景。",
  },
  { value: "chat", label: "Chat", description: "偏向通用对话、协作与解释。" },
  {
    value: "coding",
    label: "Coding",
    description: "偏向代码实现、验证与修复。",
  },
  {
    value: "research",
    label: "Research",
    description: "偏向检索、证据归纳与信息对比。",
  },
  {
    value: "planning",
    label: "Planning",
    description: "偏向路线、方案与拆解。",
  },
  {
    value: "review",
    label: "Review",
    description: "偏向评审、风险识别与反馈。",
  },
];

function profileToUiValue(
  value: PromptControlSettings["default_scenario_profile"],
): PromptScenarioProfile | "auto" {
  return value ?? "auto";
}

function summarizeIdentity(
  settings: PromptControlSettings,
  catalog: PromptControlCatalog | null,
): string {
  if (!catalog) return "加载中";
  const soul = settings.default_soul_id
    ? catalog.souls.find((item) => item.id === settings.default_soul_id)
    : null;
  const persona = settings.default_persona_id
    ? catalog.personas.find((item) => item.id === settings.default_persona_id)
    : null;
  if (!soul && !persona) return "Built-in Auto Identity";
  if (soul && persona) return `${soul.name} / ${persona.name}`;
  return soul?.name ?? persona?.name ?? "Built-in Auto Identity";
}

function parseAppliedIdentity(snapshot: PromptDiagnosticsSnapshot | null): {
  soulId: string | null;
  personaId: string | null;
  source: string | null;
} {
  if (!snapshot) {
    return { soulId: null, personaId: null, source: null };
  }
  const soulEntry = snapshot.summary.activated_entries.find((entry) =>
    entry.entry_id.startsWith("identity:soul@"),
  );
  const personaEntry = snapshot.summary.activated_entries.find((entry) =>
    entry.entry_id.startsWith("identity:persona@"),
  );
  return {
    soulId: soulEntry?.entry_id.split("@")[1] ?? null,
    personaId: personaEntry?.entry_id.split("@")[1] ?? null,
    source: soulEntry?.source ?? personaEntry?.source ?? null,
  };
}

function parseAppliedIdentityCustomization(
  snapshot: PromptDiagnosticsSnapshot | null,
): {
  soulCustomized: boolean;
  personaCustomized: boolean;
  customizedEntryIds: string[];
} {
  if (!snapshot) {
    return {
      soulCustomized: false,
      personaCustomized: false,
      customizedEntryIds: [],
    };
  }
  const customizedReasons = snapshot.summary.activation_reasons.filter(
    (reason) =>
      reason.lane === "identity" &&
      reason.reason_code === "custom_identity_pack_applied",
  );
  return {
    soulCustomized: customizedReasons.some((reason) =>
      reason.entry_id.startsWith("identity:soul@"),
    ),
    personaCustomized: customizedReasons.some((reason) =>
      reason.entry_id.startsWith("identity:persona@"),
    ),
    customizedEntryIds: customizedReasons.map((reason) => reason.entry_id),
  };
}

type IdentityDiffItem = {
  scope: "Soul" | "Persona";
  field: string;
  builtInValue: string;
  appliedValue: string;
};

const SOUL_FIELD_LABELS: Record<string, string> = {
  summary: "Summary",
  mission: "Mission",
  core_principles: "Core Principles",
  decision_contract: "Decision Contract",
  non_negotiables: "Non-negotiables",
};

const PERSONA_FIELD_LABELS: Record<string, string> = {
  summary: "Summary",
  tone_rules: "Tone Rules",
  collaboration_rules: "Collaboration Rules",
  output_preferences: "Output Preferences",
};

function formatIdentityFieldValue(
  value: string | string[] | undefined,
): string {
  if (Array.isArray(value)) {
    return value.length > 0 ? value.join(" / ") : "empty";
  }
  const trimmed = value?.trim() ?? "";
  return trimmed.length > 0 ? trimmed : "empty";
}

function collectIdentityDiff(
  appliedIdentity: { soulId: string | null; personaId: string | null },
  catalog: PromptControlCatalog | null,
  pack: IdentityCustomizationPack,
): IdentityDiffItem[] {
  if (!catalog) return [];

  const diffs: IdentityDiffItem[] = [];
  const builtInSoul = appliedIdentity.soulId
    ? (catalog.souls.find((item) => item.id === appliedIdentity.soulId) ?? null)
    : null;
  const builtInPersona = appliedIdentity.personaId
    ? (catalog.personas.find((item) => item.id === appliedIdentity.personaId) ??
      null)
    : null;
  const soulOverride =
    (appliedIdentity.soulId && pack.souls[appliedIdentity.soulId]) || null;
  const personaOverride =
    (appliedIdentity.personaId && pack.personas[appliedIdentity.personaId]) ||
    null;

  if (builtInSoul && soulOverride) {
    Object.entries(SOUL_FIELD_LABELS).forEach(([field, label]) => {
      const overrideValue = soulOverride[field as keyof typeof soulOverride];
      if (overrideValue === undefined || overrideValue === null) return;
      diffs.push({
        scope: "Soul",
        field: label,
        builtInValue: formatIdentityFieldValue(
          builtInSoul[field as keyof typeof builtInSoul] as string | string[],
        ),
        appliedValue: formatIdentityFieldValue(
          overrideValue as string | string[],
        ),
      });
    });
  }

  if (builtInPersona && personaOverride) {
    Object.entries(PERSONA_FIELD_LABELS).forEach(([field, label]) => {
      const overrideValue =
        personaOverride[field as keyof typeof personaOverride];
      if (overrideValue === undefined || overrideValue === null) return;
      diffs.push({
        scope: "Persona",
        field: label,
        builtInValue: formatIdentityFieldValue(
          builtInPersona[field as keyof typeof builtInPersona] as
            | string
            | string[],
        ),
        appliedValue: formatIdentityFieldValue(
          overrideValue as string | string[],
        ),
      });
    });
  }

  return diffs;
}

function parseAppliedScenario(snapshot: PromptDiagnosticsSnapshot | null): {
  scenario: string | null;
  source: string | null;
} {
  if (!snapshot) return { scenario: null, source: null };
  const scenarioEntry = snapshot.summary.activated_entries.find(
    (entry) => entry.lane === "scenario",
  );
  return {
    scenario: scenarioEntry?.entry_id.split(":")[1] ?? null,
    source: scenarioEntry?.source ?? null,
  };
}

function collectAppliedToolPolicies(
  snapshot: PromptDiagnosticsSnapshot | null,
): string[] {
  if (!snapshot) return [];
  return snapshot.summary.activated_entries
    .filter((entry) => entry.lane === "tool_policy")
    .map((entry) => entry.entry_id.replace("tool_policy:", ""));
}

type ToolPolicyDetail = {
  catalogKey: string;
  source: string;
  reasons: Array<{
    reasonCode: string;
    detail: string;
  }>;
};

function humanizeKey(value: string): string {
  return value
    .split(/[_:-]/g)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatClassifierSlotSummary(
  summary: unknown,
): Array<{ key: string; value: string }> {
  if (!summary || typeof summary !== "object" || Array.isArray(summary))
    return [];
  return Object.entries(summary as Record<string, unknown>)
    .filter(
      ([, value]) => value !== null && value !== undefined && value !== "",
    )
    .map(([key, value]) => ({
      key,
      value:
        typeof value === "string"
          ? value
          : typeof value === "number" || typeof value === "boolean"
            ? String(value)
            : JSON.stringify(value),
    }));
}

function collectToolPolicyDetails(
  snapshot: PromptDiagnosticsSnapshot | null,
): ToolPolicyDetail[] {
  if (!snapshot) return [];
  const reasonsByEntry = snapshot.summary.activation_reasons
    .filter((reason) => reason.lane === "tool_policy")
    .reduce<Record<string, ToolPolicyDetail["reasons"]>>((acc, reason) => {
      acc[reason.entry_id] = acc[reason.entry_id] ?? [];
      acc[reason.entry_id].push({
        reasonCode: reason.reason_code,
        detail: reason.detail,
      });
      return acc;
    }, {});

  return snapshot.summary.activated_entries
    .filter((entry) => entry.lane === "tool_policy")
    .map((entry) => ({
      catalogKey: entry.entry_id.replace("tool_policy:", ""),
      source: entry.source,
      reasons: reasonsByEntry[entry.entry_id] ?? [],
    }));
}

function humanizeScenarioLabel(value: string | null): string {
  if (!value) return "未记录";
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function readLastActiveSessionId(): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem("lastActiveSessionId");
  } catch {
    return null;
  }
}

const controlClass =
  "h-8 w-full rounded-lg border border-black/[0.09] bg-black/[0.025] px-3 text-[12.5px] font-medium text-foreground/85 outline-none transition-all focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15";

function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  );
}

function CardHeader({
  eyebrow,
  title,
  description,
}: {
  eyebrow: string;
  title: string;
  description?: string;
}) {
  return (
    <div>
      <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
        {eyebrow}
      </div>
      <h3 className="mt-1 text-[14px] font-semibold tracking-tight text-foreground/90">
        {title}
      </h3>
      {description ? (
        <p className="mt-0.5 text-[11.5px] leading-4 text-muted-foreground">
          {description}
        </p>
      ) : null}
    </div>
  );
}

function MiniStat({
  label,
  value,
  hint,
  mono,
}: {
  label: string;
  value: string;
  hint?: string;
  mono?: boolean;
}) {
  return (
    <div className="rounded-lg border border-black/[0.06] bg-black/[0.015] px-3 py-2">
      <div className="text-[10.5px] font-medium text-black/40">{label}</div>
      <div
        className={cn(
          "mt-0.5 text-[12.5px] font-semibold text-foreground/88",
          mono && "font-mono text-[11.5px]",
        )}
      >
        {value}
      </div>
      {hint ? (
        <div className="mt-0.5 text-[11px] leading-4 text-muted-foreground">{hint}</div>
      ) : null}
    </div>
  );
}

function ColumnCard({
  label,
  tone = "default",
  children,
}: {
  label: string;
  tone?: "default" | "muted" | "sky" | "emerald";
  children: ReactNode;
}) {
  const toneClass =
    tone === "muted"
      ? "border-black/[0.06] bg-black/[0.02]"
      : tone === "sky"
        ? "border-sky-200/50 bg-sky-50/30"
        : tone === "emerald"
          ? "border-emerald-200/50 bg-emerald-50/25"
          : "border-black/[0.06] bg-white";
  return (
    <section className={cn("rounded-xl border px-3.5 py-3", toneClass)}>
      <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/35">
        {label}
      </div>
      <div className="mt-2.5 flex flex-col gap-2.5">{children}</div>
    </section>
  );
}

function ColField({
  label,
  value,
  hint,
  children,
}: {
  label: string;
  value: string;
  hint?: string;
  children?: ReactNode;
}) {
  return (
    <div>
      <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
        {label}
      </div>
      <div className="mt-0.5 text-[12.5px] font-semibold text-foreground/88">{value}</div>
      {hint ? (
        <div className="mt-0.5 text-[11px] leading-4 text-muted-foreground">{hint}</div>
      ) : null}
      {children}
    </div>
  );
}

function Pill({
  tone = "neutral",
  children,
}: {
  tone?: "neutral" | "amber" | "sky" | "emerald";
  children: ReactNode;
}) {
  const toneClass =
    tone === "amber"
      ? "border-amber-200/70 bg-amber-50 text-amber-800"
      : tone === "sky"
        ? "border-sky-200/70 bg-sky-50 text-sky-800"
        : tone === "emerald"
          ? "border-emerald-200/70 bg-emerald-50 text-emerald-800"
          : "border-black/[0.07] bg-black/[0.03] text-muted-foreground";
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-1.5 py-0.5 text-[10px] font-medium tracking-tight",
        toneClass,
      )}
    >
      {children}
    </span>
  );
}

export function PromptDiagnosticsPage() {
  const [snapshot, setSnapshot] = useState<PromptDiagnosticsSnapshot | null>(
    null,
  );
  const [catalog, setCatalog] = useState<PromptControlCatalog | null>(null);
  const [identityPack, setIdentityPack] = useState<IdentityCustomizationPack>({
    souls: {},
    personas: {},
  });
  const [currentSession, setCurrentSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [selectedProfile, setSelectedProfile] = useState<
    PromptScenarioProfile | "auto"
  >("auto");
  const [promptDiagnosticsEnabled, setPromptDiagnosticsEnabled] =
    useState(true);
  const [selectedSoulId, setSelectedSoulId] = useState<string>("auto");
  const [selectedPersonaId, setSelectedPersonaId] = useState<string>("auto");
  // Preserve agent_name / user_name across saves on this page (we don't
  // edit them here — that lives on AgentIdentitySettingsPage — but we
  // must round-trip them or set_prompt_control_settings would clear them.
  const [persistedAgentName, setPersistedAgentName] = useState<string | null>(
    null,
  );
  const [persistedUserName, setPersistedUserName] = useState<string | null>(
    null,
  );

  const refreshDiagnostics = () => {
    setSnapshot(readLatestPromptDiagnosticsSnapshot());
  };

  const loadControlSettings = async () => {
    setLoading(true);
    try {
      const [settings, nextCatalog, nextIdentityPack] = await Promise.all([
        getPromptControlSettings(),
        getPromptControlCatalog(),
        getIdentityPack(),
      ]);
      setCatalog(nextCatalog);
      setIdentityPack(nextIdentityPack);
      setSelectedProfile(profileToUiValue(settings.default_scenario_profile));
      setPromptDiagnosticsEnabled(settings.prompt_diagnostics_enabled);
      setSelectedSoulId(settings.default_soul_id ?? "auto");
      setSelectedPersonaId(settings.default_persona_id ?? "auto");
      setPersistedAgentName(settings.agent_name ?? null);
      setPersistedUserName(settings.user_name ?? null);
    } catch (error) {
      toast.error("读取 Prompt Control 设置失败", {
        description: String(error),
      });
    } finally {
      setLoading(false);
    }
  };

  const loadCurrentSession = async () => {
    const activeSessionId = readLastActiveSessionId();
    if (!activeSessionId) {
      setCurrentSession(null);
      return;
    }
    try {
      setCurrentSession(await getSession(activeSessionId));
    } catch {
      setCurrentSession(null);
    }
  };

  useEffect(() => {
    refreshDiagnostics();
    void loadControlSettings();
    void loadCurrentSession();
  }, []);

  useCrossWindowChange<PromptDiagnosticsSnapshot | null>(
    "cross:prompt-diagnostics-changed",
    (payload) => {
      if (payload) {
        setSnapshot(payload);
      } else {
        refreshDiagnostics();
      }
    },
  );

  useCrossWindowChange<{ sessionId: string }>(
    "cross:session-identity-changed",
    () => {
      void loadCurrentSession();
    },
  );

  const selectedProfileMeta = useMemo(
    () =>
      PROFILE_OPTIONS.find((option) => option.value === selectedProfile) ??
      PROFILE_OPTIONS[0],
    [selectedProfile],
  );
  const appliedIdentity = useMemo(
    () => parseAppliedIdentity(snapshot),
    [snapshot],
  );
  const appliedScenario = useMemo(
    () => parseAppliedScenario(snapshot),
    [snapshot],
  );
  const appliedIdentityCustomization = useMemo(
    () => parseAppliedIdentityCustomization(snapshot),
    [snapshot],
  );
  const identityDiff = useMemo(
    () => collectIdentityDiff(appliedIdentity, catalog, identityPack),
    [appliedIdentity, catalog, identityPack],
  );
  const appliedToolPolicies = useMemo(
    () => collectAppliedToolPolicies(snapshot),
    [snapshot],
  );
  const appliedToolPolicyDetails = useMemo(
    () => collectToolPolicyDetails(snapshot),
    [snapshot],
  );
  const classifierDecision = useRuntimeProjectionSelector(
    (state) => state.executionMode,
  );
  const classifierSlotEvidence = useMemo(
    () => formatClassifierSlotSummary(classifierDecision?.slotSummary ?? null),
    [classifierDecision?.slotSummary],
  );

  const availablePersonas = useMemo(() => {
    if (!catalog) return [];
    if (selectedSoulId === "auto") return catalog.personas;
    return catalog.personas.filter(
      (persona) => persona.soul_id === selectedSoulId,
    );
  }, [catalog, selectedSoulId]);

  const effectiveSettings = useMemo<PromptControlSettings>(
    () => ({
      default_scenario_profile:
        selectedProfile === "auto" ? null : selectedProfile,
      default_soul_id: selectedSoulId === "auto" ? null : selectedSoulId,
      default_persona_id:
        selectedPersonaId === "auto" ? null : selectedPersonaId,
      agent_name: persistedAgentName,
      user_name: persistedUserName,
      prompt_diagnostics_enabled: promptDiagnosticsEnabled,
    }),
    [
      promptDiagnosticsEnabled,
      selectedPersonaId,
      selectedProfile,
      selectedSoulId,
      persistedAgentName,
      persistedUserName,
    ],
  );

  const handleSoulChange = (nextSoulId: string) => {
    const nextPersonas =
      nextSoulId === "auto"
        ? (catalog?.personas ?? [])
        : (catalog?.personas ?? []).filter(
            (persona) => persona.soul_id === nextSoulId,
          );
    setSelectedSoulId(nextSoulId);
    if (nextSoulId === "auto") {
      setSelectedPersonaId("auto");
      return;
    }
    const personaStillValid = nextPersonas.some(
      (persona) => persona.id === selectedPersonaId,
    );
    if (!personaStillValid) {
      setSelectedPersonaId("auto");
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await setPromptControlSettings({
        default_scenario_profile: effectiveSettings.default_scenario_profile,
        default_soul_id: effectiveSettings.default_soul_id,
        default_persona_id: effectiveSettings.default_persona_id,
        agent_name: effectiveSettings.agent_name,
        user_name: effectiveSettings.user_name,
        prompt_diagnostics_enabled:
          effectiveSettings.prompt_diagnostics_enabled,
      });
      toast.success("Prompt Control 已保存", {
        description: "配置已写入 ~/.if2ai/prompt/control-plane.json",
      });
      await loadControlSettings();
    } catch (error) {
      toast.error("保存 Prompt Control 失败", { description: String(error) });
    } finally {
      setSaving(false);
    }
  };

  const handleUpdateCurrentSessionIdentity = async (
    sessionId: string,
    identity: { soul_id?: string | null; persona_id?: string | null },
  ) => {
    const updated = await updateSessionIdentity(sessionId, identity);
    await broadcastChange("cross:session-identity-changed", {
      sessionId,
      soulId: updated.soul_id ?? null,
      personaId: updated.persona_id ?? null,
    });
    await loadCurrentSession();
  };

  return (
    <div className="flex flex-col gap-3 text-foreground">
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <SectionLabel>Prompt Control Plane</SectionLabel>
            <h2 className="mt-1.5 text-[15px] font-semibold tracking-tight text-foreground">
              Prompt 诊断面板
            </h2>
            <p className="mt-1 max-w-2xl text-[11.5px] leading-5 text-muted-foreground">
              实时查看 Prompt Control Plane 的运行状态、Identity 差异、Classifier
              证据、Tool Policy 决策与 Memory 召回详情。
            </p>
          </div>
          <div className="inline-flex items-center gap-1.5 rounded-lg border border-black/[0.07] bg-jade/8 px-2 py-1 text-[10px] font-medium tracking-tight text-jade">
            <span className="h-1.5 w-1.5 rounded-full bg-jade" />
            Runtime
          </div>
        </div>
      </SettingsSurface>

      <div className="grid gap-3 lg:grid-cols-[minmax(0,1.45fr)_minmax(17rem,0.9fr)]">
        <div className="flex flex-col gap-3">
          <SettingsSurface className="px-5 py-4">
            <div className="flex items-center justify-between gap-3">
              <SectionLabel>默认场景 Profile</SectionLabel>
              <span className="rounded-md border border-black/[0.07] bg-black/[0.025] px-1.5 py-0.5 text-[10px] font-medium text-black/55">
                {selectedProfileMeta.label}
              </span>
            </div>
            <p className="-mt-2 mb-3 text-[11.5px] leading-4 text-muted-foreground">
              classifier 未明确给出场景时，用此 profile 填补 scenario lane。
            </p>
            <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {PROFILE_OPTIONS.map((option) => {
                const active = selectedProfile === option.value;
                return (
                  <button
                    key={option.value}
                    type="button"
                    onClick={() => setSelectedProfile(option.value)}
                    className={cn(
                      "rounded-xl border px-3 py-2.5 text-left transition-all",
                      active
                        ? "border-jade/40 bg-jade/[0.06] shadow-[0_0_0_3px_rgba(16,185,129,0.06)]"
                        : "border-black/[0.06] bg-black/[0.015] hover:border-black/[0.1] hover:bg-black/[0.025]",
                    )}
                  >
                    <div className="text-[12.5px] font-semibold tracking-tight text-foreground/85">
                      {option.label}
                    </div>
                    <div className="mt-0.5 text-[11px] leading-4 text-muted-foreground">
                      {option.description}
                    </div>
                  </button>
                );
              })}
            </div>
          </SettingsSurface>

          <SettingsSurface className="px-5 py-4">
            <SectionLabel>Identity 默认值</SectionLabel>
            <p className="-mt-2 mb-3 text-[11.5px] leading-4 text-muted-foreground">
              全局默认 Soul / Persona。resolver 会确保 Persona 只能挂在对应 Soul 下，非法组合会被自动收敛。
            </p>
            <div className="grid gap-3 sm:grid-cols-2">
              <label className="grid gap-1.5">
                <span className="text-[11px] font-medium text-black/40">Soul</span>
                <select
                  value={selectedSoulId}
                  onChange={(event) => handleSoulChange(event.target.value)}
                  className={controlClass}
                >
                  <option value="auto">Auto / Built-in</option>
                  {(catalog?.souls ?? []).map((soul) => (
                    <option key={soul.id} value={soul.id}>
                      {soul.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="grid gap-1.5">
                <span className="text-[11px] font-medium text-black/40">Persona</span>
                <select
                  value={selectedPersonaId}
                  onChange={(event) => setSelectedPersonaId(event.target.value)}
                  className={controlClass}
                >
                  <option value="auto">Auto / None</option>
                  {availablePersonas.map((persona) => (
                    <option key={persona.id} value={persona.id}>
                      {persona.name}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="mt-3 grid gap-2 sm:grid-cols-2">
              <MiniStat
                label="Selected Soul"
                value={
                  selectedSoulId === "auto"
                    ? "Built-in default"
                    : (catalog?.souls.find((soul) => soul.id === selectedSoulId)?.name ??
                      selectedSoulId)
                }
              />
              <MiniStat
                label="Selected Persona"
                value={
                  selectedPersonaId === "auto"
                    ? "Auto / none"
                    : (availablePersonas.find((persona) => persona.id === selectedPersonaId)
                        ?.name ?? selectedPersonaId)
                }
              />
            </div>
          </SettingsSurface>

          <SettingsSurface className="px-5 py-4">
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0">
                <SectionLabel>诊断流</SectionLabel>
                <p className="-mt-2 text-[11.5px] leading-4 text-muted-foreground">
                  控制 <code className="rounded bg-black/[0.04] px-1 py-0.5 text-[10.5px]">stream_complete</code> 是否下发 prompt diagnostics summary。
                </p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={promptDiagnosticsEnabled}
                onClick={() => setPromptDiagnosticsEnabled((value) => !value)}
                className={cn(
                  "relative h-6 w-11 shrink-0 rounded-full transition-colors",
                  promptDiagnosticsEnabled ? "bg-jade" : "bg-black/15",
                )}
              >
                <span
                  className={cn(
                    "absolute top-0.5 h-5 w-5 rounded-full bg-white shadow-sm transition-transform",
                    promptDiagnosticsEnabled ? "translate-x-5" : "translate-x-0.5",
                  )}
                />
              </button>
            </div>
          </SettingsSurface>
        </div>

        <SettingsSurface className="px-5 py-4">
          <div className="flex items-center justify-between gap-2">
            <SectionLabel>Runtime Projection</SectionLabel>
            <span className="inline-flex items-center gap-1.5 text-[10px] font-medium text-jade">
              <span className="h-1.5 w-1.5 rounded-full bg-jade" />
              Live
            </span>
          </div>
          <div className="mt-1 flex flex-col gap-2">
            <MiniStat label="Config Path" value="~/.if2ai/prompt/control-plane.json" mono />
            <MiniStat label="Identity" value={summarizeIdentity(effectiveSettings, catalog)} />
            <MiniStat
              label="Scenario Default"
              value={selectedProfileMeta.label}
              hint={selectedProfileMeta.description}
            />
            <MiniStat
              label="Diagnostics"
              value={promptDiagnosticsEnabled ? "Enabled" : "Disabled"}
              hint={
                promptDiagnosticsEnabled
                  ? "新的 turn 会继续把 prompt summary 投影到 diagnostics。"
                  : "新的 turn 不再向前端发送 prompt diagnostics summary。"
              }
            />
          </div>
          <div className="mt-3 flex items-center justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 rounded-lg border-black/[0.09] bg-black/[0.025] px-3 text-[11.5px] shadow-none hover:bg-black/[0.05]"
              onClick={() => {
                void loadControlSettings();
                refreshDiagnostics();
                void loadCurrentSession();
              }}
            >
              刷新
            </Button>
            <Button
              type="button"
              size="sm"
              className="h-7 rounded-lg px-3 text-[11.5px]"
              disabled={loading || saving}
              onClick={() => void handleSave()}
            >
              {saving ? "保存中..." : "保存配置"}
            </Button>
          </div>
        </SettingsSurface>
      </div>

      <SettingsSurface className="px-5 py-4">
        <CardHeader
          eyebrow="Session Identity"
          title="当前会话 Identity"
          description="控制最近一次主窗口选中的 session override。Persona 是常规切换入口，Soul 放在高级入口中，且只影响当前会话后续回复。"
        />
        <SessionIdentityEditor
          session={
            currentSession
              ? {
                  id: currentSession.id,
                  title: currentSession.title,
                  soul_id: currentSession.soul_id ?? null,
                  persona_id: currentSession.persona_id ?? null,
                }
              : null
          }
          catalog={catalog}
          onUpdate={handleUpdateCurrentSessionIdentity}
        />
      </SettingsSurface>

      <SettingsSurface className="px-5 py-4">
        <CardHeader
          eyebrow="Effective Snapshot"
          title="保存默认 vs 最近一次实际采用"
          description="左侧是当前保存到 Prompt Control Plane 的默认值；右侧是最近一次成功投影到 diagnostics 的 turn 实际采用值。"
        />
        <div className="mt-3 grid gap-3 lg:grid-cols-2">
          <ColumnCard label="Saved Defaults" tone="muted">
            <ColField label="Identity" value={summarizeIdentity(effectiveSettings, catalog)} />
            <ColField label="Scenario" value={selectedProfileMeta.label} />
            <ColField
              label="Diagnostics Policy"
              value={
                promptDiagnosticsEnabled
                  ? "Project summaries to frontend"
                  : "Do not project new summaries"
              }
            />
          </ColumnCard>

          <ColumnCard label="Latest Applied Turn">
            <ColField
              label="Identity"
              value={
                snapshot
                  ? `${appliedIdentity.soulId ?? "unknown soul"}${appliedIdentity.personaId ? ` / ${appliedIdentity.personaId}` : ""}`
                  : "暂无最新 turn"
              }
              hint={appliedIdentity.source ? `source: ${appliedIdentity.source}` : undefined}
            >
              {snapshot ? (
                <div className="mt-1.5 flex flex-wrap gap-1.5">
                  <Pill
                    tone={
                      appliedIdentityCustomization.soulCustomized ||
                      appliedIdentityCustomization.personaCustomized
                        ? "amber"
                        : "neutral"
                    }
                  >
                    {appliedIdentityCustomization.soulCustomized ||
                    appliedIdentityCustomization.personaCustomized
                      ? "Custom Pack"
                      : "Built-in"}
                  </Pill>
                  {appliedIdentityCustomization.soulCustomized ? (
                    <Pill tone="amber">Soul override</Pill>
                  ) : null}
                  {appliedIdentityCustomization.personaCustomized ? (
                    <Pill tone="amber">Persona override</Pill>
                  ) : null}
                </div>
              ) : null}
            </ColField>
            <ColField
              label="Scenario"
              value={snapshot ? humanizeScenarioLabel(appliedScenario.scenario) : "暂无最新 turn"}
              hint={appliedScenario.source ? `source: ${appliedScenario.source}` : undefined}
            />
            <div className="rounded-xl border border-amber-200/60 bg-amber-50/40 px-3 py-2.5">
              <div className="flex items-center justify-between gap-2">
                <div className="text-[10.5px] font-semibold uppercase tracking-widest text-amber-800/75">
                  Identity Diff
                </div>
                <span className="rounded-md border border-amber-200/70 bg-white/70 px-1.5 py-0.5 text-[10px] font-medium text-amber-800">
                  vs Built-in
                </span>
              </div>
              {identityDiff.length === 0 ? (
                <div className="mt-1.5 text-[11.5px] leading-5 text-muted-foreground">
                  {snapshot
                    ? "当前 turn 未发现 identity pack 对 built-in 字段的覆盖。"
                    : "暂无最新 turn"}
                </div>
              ) : (
                <div className="mt-2 space-y-1.5">
                  {identityDiff.map((item) => (
                    <div
                      key={`${item.scope}-${item.field}`}
                      className="rounded-lg border border-white/95 bg-white/80 px-2.5 py-2"
                    >
                      <div className="flex flex-wrap items-center gap-1.5">
                        <Pill tone="amber">{item.scope}</Pill>
                        <span className="text-[11.5px] font-semibold text-foreground/85">
                          {item.field}
                        </span>
                      </div>
                      <div className="mt-1.5 grid gap-1.5 md:grid-cols-2">
                        <div className="rounded-md border border-black/[0.05] bg-black/[0.02] px-2 py-1.5">
                          <div className="text-[10px] font-medium uppercase tracking-widest text-muted-foreground/75">
                            Built-in
                          </div>
                          <div className="mt-0.5 text-[11.5px] leading-4 text-foreground/75">
                            {item.builtInValue}
                          </div>
                        </div>
                        <div className="rounded-md border border-amber-200/70 bg-amber-50/70 px-2 py-1.5">
                          <div className="text-[10px] font-medium uppercase tracking-widest text-amber-800/80">
                            Applied
                          </div>
                          <div className="mt-0.5 text-[11.5px] leading-4 text-foreground/85">
                            {item.appliedValue}
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
            <div>
              <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
                Tool Policy Catalog
              </div>
              {appliedToolPolicies.length === 0 ? (
                <div className="mt-1 text-[12.5px] font-medium text-foreground/80">
                  {snapshot ? "该 turn 未激活 tool policy lane" : "暂无最新 turn"}
                </div>
              ) : (
                <div className="mt-1.5 flex flex-wrap gap-1.5">
                  {appliedToolPolicies.map((policy) => (
                    <Pill key={policy} tone="sky">
                      {policy}
                    </Pill>
                  ))}
                </div>
              )}
            </div>
            <ColField
              label="Diagnostics Policy"
              value={
                snapshot
                  ? `projected at ${new Date(snapshot.updatedAt).toLocaleString("zh-CN", { hour12: false })}`
                  : "暂无已投影的诊断快照"
              }
            />
          </ColumnCard>
        </div>
      </SettingsSurface>

      <SettingsSurface className="px-5 py-4">
        <CardHeader
          eyebrow="Override vs Evidence"
          title="Scenario / Identity / Tool Policy Diff"
          description="把手动默认、classifier evidence 与最近一次 turn 实际结果并排展示，便于判断 override 是否生效、classifier 提示了什么、planner 最终落点。"
        />
        <div className="mt-3 grid gap-3 xl:grid-cols-3">
          <ColumnCard label="Manual / Saved Defaults" tone="muted">
            <ColField
              label="Identity"
              value={summarizeIdentity(effectiveSettings, catalog)}
              hint="Soul / Persona 是显式默认值，不由 classifier 自动产出。"
            />
            <ColField
              label="Scenario"
              value={selectedProfileMeta.label}
              hint={
                selectedProfile === "auto"
                  ? "未设置硬默认，优先采用 classifier hint。"
                  : "classifier 未给出明确场景时，作为 fallback。"
              }
            />
            <ColField
              label="Tool Policy"
              value="No manual catalog override"
              hint="tool policy 仍由 planner / tool registry / request intelligence 联合决定。"
            />
          </ColumnCard>

          <ColumnCard label="Classifier Evidence" tone="sky">
            <ColField
              label="Scenario Hint"
              value={
                classifierDecision?.scenarioProfileHint
                  ? humanizeScenarioLabel(classifierDecision.scenarioProfileHint)
                  : "暂无 classifier scenario hint"
              }
              hint={`mode: ${classifierDecision?.executionMode ?? "未记录"} · risk: ${classifierDecision?.riskLevel ?? "未记录"} · complexity: ${classifierDecision?.complexityLevel ?? "未记录"}`}
            />
            <div>
              <div className="flex items-center justify-between gap-2">
                <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
                  Matched Rules
                </div>
                {classifierDecision?.policyVersion ? (
                  <Pill tone="sky">{classifierDecision.policyVersion}</Pill>
                ) : null}
              </div>
              {classifierDecision?.matchedRules.length ? (
                <div className="mt-1.5 flex flex-wrap gap-1.5">
                  {classifierDecision.matchedRules.map((ruleId) => (
                    <Pill key={ruleId} tone="sky">
                      {ruleId}
                    </Pill>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-[11.5px] text-muted-foreground">
                  暂无 matched rule evidence
                </div>
              )}
            </div>
            <div>
              <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
                Classifier Slots
              </div>
              {classifierSlotEvidence.length ? (
                <div className="mt-1.5 space-y-1">
                  {classifierSlotEvidence.map((item) => (
                    <div
                      key={item.key}
                      className="flex items-start justify-between gap-3 rounded-md border border-black/[0.06] bg-black/[0.02] px-2.5 py-1.5"
                    >
                      <div className="text-[11px] font-medium text-foreground/78">
                        {humanizeKey(item.key)}
                      </div>
                      <div className="max-w-[68%] text-right text-[11px] leading-4 text-muted-foreground">
                        {item.value}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-[11.5px] text-muted-foreground">
                  暂无 classifier slot summary
                </div>
              )}
              {classifierDecision?.ambiguousEscalated ? (
                <div className="mt-2 rounded-md border border-amber-200/70 bg-amber-50/70 px-2.5 py-1.5 text-[11px] leading-4 text-amber-800">
                  classifier ambiguity escalated
                  {classifierDecision.escalationSource
                    ? ` · source: ${classifierDecision.escalationSource}`
                    : ""}
                </div>
              ) : null}
            </div>
          </ColumnCard>

          <ColumnCard label="Latest Applied Turn" tone="emerald">
            <ColField
              label="Identity"
              value={
                snapshot
                  ? `${appliedIdentity.soulId ?? "unknown soul"}${appliedIdentity.personaId ? ` / ${appliedIdentity.personaId}` : ""}`
                  : "暂无最新 turn"
              }
              hint={
                appliedIdentity.source
                  ? `resolved from ${appliedIdentity.source}`
                  : "等待新的 diagnostics snapshot"
              }
            />
            <ColField
              label="Scenario"
              value={snapshot ? humanizeScenarioLabel(appliedScenario.scenario) : "暂无最新 turn"}
              hint={
                appliedScenario.source
                  ? `selected by ${appliedScenario.source}`
                  : "等待新的 diagnostics snapshot"
              }
            />
            <div>
              <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
                Tool Policy Catalog
              </div>
              {appliedToolPolicies.length === 0 ? (
                <div className="mt-1 text-[12.5px] font-medium text-foreground/80">
                  {snapshot ? "该 turn 未激活 tool policy lane" : "暂无最新 turn"}
                </div>
              ) : (
                <div className="mt-1.5 flex flex-wrap gap-1.5">
                  {appliedToolPolicies.map((policy) => (
                    <Pill key={policy} tone="emerald">
                      {policy}
                    </Pill>
                  ))}
                </div>
              )}
            </div>
          </ColumnCard>
        </div>
      </SettingsSurface>

      <SettingsSurface className="px-5 py-4">
        <CardHeader
          eyebrow="Tool Policy Detail"
          title="Effective Tool Policy"
          description="展示最近一个已投影 turn 中真正激活的 tool policy catalog，并把 activation reasons 按 catalog 聚合。"
        />
        {appliedToolPolicyDetails.length === 0 ? (
          <div className="mt-3 rounded-xl border border-dashed border-black/[0.08] bg-black/[0.02] px-4 py-6 text-center text-[12px] text-muted-foreground">
            {snapshot
              ? "最近一个 turn 没有激活 tool policy catalog。"
              : "完成一次 assistant turn 后，这里会显示最新的 effective tool policy detail。"}
          </div>
        ) : (
          <div className="mt-3 grid gap-3 lg:grid-cols-2">
            {appliedToolPolicyDetails.map((policy) => (
              <section
                key={policy.catalogKey}
                className="rounded-xl border border-black/[0.06] bg-black/[0.015] px-3.5 py-3"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
                      Catalog
                    </div>
                    <h4 className="mt-0.5 truncate text-[13px] font-semibold tracking-tight text-foreground/85">
                      {humanizeKey(policy.catalogKey)}
                    </h4>
                  </div>
                  <Pill tone="sky">{policy.source}</Pill>
                </div>
                <div className="mt-2.5 space-y-1.5">
                  {policy.reasons.length === 0 ? (
                    <div className="rounded-md border border-dashed border-black/[0.08] bg-black/[0.02] px-3 py-2 text-[11.5px] text-muted-foreground">
                      该 catalog 暂无结构化 reason detail。
                    </div>
                  ) : (
                    policy.reasons.map((reason, index) => (
                      <div
                        key={`${policy.catalogKey}-${reason.reasonCode}-${index}`}
                        className="rounded-md border border-black/[0.06] bg-white px-2.5 py-2"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/40">
                            {reason.reasonCode}
                          </div>
                          <span className="rounded border border-black/[0.07] bg-black/[0.03] px-1.5 py-0.5 text-[9.5px] uppercase tracking-wider text-muted-foreground">
                            reason
                          </span>
                        </div>
                        <div className="mt-1 text-[11.5px] leading-5 text-foreground/80">
                          {reason.detail}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </section>
            ))}
          </div>
        )}
      </SettingsSurface>

      <PromptDiagnosticsPanel
        snapshot={snapshot}
        title="Latest Prompt Summary"
        description="展示最近一次 assistant turn 的 Prompt Control Plane 摘要。这里仍保持只读、结构化、可解释，不暴露原始 prompt 文本。"
      />
    </div>
  );
}

export default PromptDiagnosticsPage;
