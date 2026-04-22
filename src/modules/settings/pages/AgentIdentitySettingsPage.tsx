import { useEffect, useMemo, useState } from "react";
import { Bot, Check, Plus, RotateCcw, Shield, Sparkles, Trash2 } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { toast } from "sonner";
import {
  getIdentityCatalog,
  getIdentityDefaults,
  getIdentityPack,
  setIdentityDefaults,
  setIdentityPack,
  updateSessionIdentity,
  type IdentityCustomizationPack,
  type PromptControlCatalog,
  type PromptControlPersonaOption,
  type PromptControlSettings,
  type PromptControlSoulOption,
  type SessionIdentityInput,
} from "@/api/identity";
import { getSession } from "@/api/sessions";
import { SessionIdentityEditor } from "@/components/identity/SessionIdentityEditor";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { SettingsSurface } from "../components/SettingsSurface";
import type { Session } from "@/lib/tauri";
import { broadcastChange, useCrossWindowChange } from "@/lib/crossWindowSync";
import { cn } from "@/lib/utils";
import { PERSONA_AVATAR_BY_ID } from "@/lib/persona-avatars";
import { CreatePersonaDialog } from "@/components/identity/CreatePersonaDialog";

function readLastActiveSessionId(): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem("lastActiveSessionId");
  } catch {
    return null;
  }
}

function normalizeLines(value: string): string[] {
  return value
    .split("\n")
    .map((item) => item.replace(/^\s*[-*]\s*/, "").trim())
    .filter(Boolean);
}

function linesToTextarea(value: string[] | undefined): string {
  return (value ?? []).join("\n");
}

function arraysEqual(
  a: string[] | undefined,
  b: string[] | undefined,
): boolean {
  const left = a ?? [];
  const right = b ?? [];
  return (
    left.length === right.length &&
    left.every((item, index) => item === right[index])
  );
}

function maybeOverrideString(
  base: string | undefined,
  current: string,
): string | null {
  const trimmed = current.trim();
  const normalizedBase = (base ?? "").trim();
  if (!trimmed || trimmed === normalizedBase) return null;
  return trimmed;
}

function maybeOverrideLines(
  base: string[] | undefined,
  current: string,
): string[] | null {
  const normalized = normalizeLines(current);
  if (normalized.length === 0 || arraysEqual(base, normalized)) return null;
  return normalized;
}

function mergeCatalogWithPack(
  catalog: PromptControlCatalog | null,
  pack: IdentityCustomizationPack,
): PromptControlCatalog | null {
  if (!catalog) return null;
  return {
    souls: catalog.souls.map((soul) => {
      const overrideItem = pack.souls[soul.id];
      if (!overrideItem) return soul;
      return {
        ...soul,
        summary: overrideItem.summary ?? soul.summary,
        mission: overrideItem.mission ?? soul.mission,
        core_principles: overrideItem.core_principles ?? soul.core_principles,
        decision_contract:
          overrideItem.decision_contract ?? soul.decision_contract,
        non_negotiables: overrideItem.non_negotiables ?? soul.non_negotiables,
      };
    }),
    personas: catalog.personas.map((persona) => {
      const overrideItem = pack.personas[persona.id];
      if (!overrideItem) return persona;
      return {
        ...persona,
        summary: overrideItem.summary ?? persona.summary,
        tone_rules: overrideItem.tone_rules ?? persona.tone_rules,
        collaboration_rules:
          overrideItem.collaboration_rules ?? persona.collaboration_rules,
        output_preferences:
          overrideItem.output_preferences ?? persona.output_preferences,
      };
    }),
  };
}

const IDENTITY_PACK_SCHEMA = "if2ai.identity-pack";
const IDENTITY_PACK_VERSION = 1;

type IdentityPackFileEnvelope = IdentityCustomizationPack & {
  schema: string;
  version: number;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function validateCustomizationRecord(
  value: unknown,
  allowedKeys: string[],
  label: string,
): Record<string, unknown> {
  if (!isRecord(value)) {
    throw new Error(`${label} 必须是 JSON object`);
  }
  Object.entries(value).forEach(([entryId, entryValue]) => {
    if (!isRecord(entryValue)) {
      throw new Error(`${label}.${entryId} 必须是 JSON object`);
    }
    Object.keys(entryValue).forEach((field) => {
      if (!allowedKeys.includes(field)) {
        throw new Error(`${label}.${entryId}.${field} 不是受支持的字段`);
      }
      const fieldValue = entryValue[field];
      if (
        fieldValue !== null &&
        typeof fieldValue !== "string" &&
        !(
          Array.isArray(fieldValue) &&
          fieldValue.every((item) => typeof item === "string")
        )
      ) {
        throw new Error(`${label}.${entryId}.${field} 的类型不合法`);
      }
    });
  });
  return value;
}

function parseIdentityPackFile(raw: string): IdentityCustomizationPack {
  const parsed = JSON.parse(raw) as unknown;
  if (!isRecord(parsed)) {
    throw new Error("文件内容不是合法的 identity pack JSON object");
  }

  const souls = validateCustomizationRecord(
    parsed.souls ?? {},
    [
      "summary",
      "mission",
      "core_principles",
      "decision_contract",
      "non_negotiables",
    ],
    "souls",
  );
  const personas = validateCustomizationRecord(
    parsed.personas ?? {},
    ["summary", "tone_rules", "collaboration_rules", "output_preferences"],
    "personas",
  );

  if ("schema" in parsed || "version" in parsed) {
    if (parsed.schema !== IDENTITY_PACK_SCHEMA) {
      throw new Error(
        `identity pack schema 不匹配，期望 ${IDENTITY_PACK_SCHEMA}`,
      );
    }
    if (parsed.version !== IDENTITY_PACK_VERSION) {
      throw new Error(
        `identity pack version 不受支持，当前仅支持 v${IDENTITY_PACK_VERSION}`,
      );
    }
  }

  return {
    souls: souls as IdentityCustomizationPack["souls"],
    personas: personas as IdentityCustomizationPack["personas"],
  };
}

function buildIdentityPackFileEnvelope(
  pack: IdentityCustomizationPack,
): IdentityPackFileEnvelope {
  return {
    schema: IDENTITY_PACK_SCHEMA,
    version: IDENTITY_PACK_VERSION,
    souls: pack.souls,
    personas: pack.personas,
  };
}

function personaAccent(index: number) {
  const accents = [
    {
      start: "#d9efe3",
      end: "#91cbb0",
      stroke: "#6eaf93",
      bust: "#5f9f87",
      halo: "#f0dcc3",
    },
    {
      start: "#dfe7f5",
      end: "#8faaca",
      stroke: "#6f89ac",
      bust: "#5f7da3",
      halo: "#ebdcc8",
    },
    {
      start: "#e6e7eb",
      end: "#a5adb8",
      stroke: "#818997",
      bust: "#7a8592",
      halo: "#ede3d1",
    },
  ];
  return accents[index % accents.length];
}

function PersonaPortrait({
  label,
  index,
  personaId,
  avatarId,
  size = 96,
}: {
  label: string;
  index: number;
  personaId?: string;
  avatarId?: string | null;
  size?: number;
}) {
  // Avatar lookup: avatar_id (custom personas / explicit override)
  // wins over personaId (built-in implicit lookup).
  const avatarSrc =
    (avatarId && PERSONA_AVATAR_BY_ID[avatarId]) ||
    (personaId ? PERSONA_AVATAR_BY_ID[personaId] : undefined);
  const accent = personaAccent(index);

  if (avatarSrc) {
    return (
      <div
        className="relative mx-auto overflow-hidden rounded-full bg-[#f5efe5] ring-1 ring-black/[0.06] shadow-[0_6px_18px_rgba(15,23,42,0.08),inset_0_0_0_3px_rgba(255,255,255,0.85)]"
        style={{ height: size, width: size }}
        aria-label={label}
      >
        <img
          src={avatarSrc}
          alt={label}
          className="h-full w-full object-cover object-[50%_28%] select-none"
          draggable={false}
        />
      </div>
    );
  }

  return (
    <div
      className="relative mx-auto"
      style={{ height: size, width: size }}
      aria-label={label}
    >
      <div
        className="absolute inset-0 rounded-full border"
        style={{
          background: `radial-gradient(circle at 50% 42%, ${accent.halo} 0%, #f8f3eb 50%, #efe2cf 100%)`,
          borderColor: "rgba(188,167,145,0.55)",
          boxShadow: "inset 0 0 0 2px rgba(255,255,255,0.7)",
        }}
      />
      <svg viewBox="0 0 120 120" className="absolute inset-0 h-full w-full">
        <path
          d="M39 92c3-16 16-24 31-24 14 0 26 7 31 24"
          fill={accent.bust}
          fillOpacity="0.92"
        />
        <ellipse cx="62" cy="47" rx="18" ry="20" fill={accent.bust} fillOpacity="0.96" />
        <path
          d="M41 44c1-11 9-21 22-23 12-2 23 4 29 15-3-2-8-3-14-2-4 1-8 0-12-2-5-3-11-2-16 0-3 1-6 4-9 12Z"
          fill={accent.bust}
        />
      </svg>
    </div>
  );
}

function SectionHeader({
  eyebrow,
  title,
  description,
}: {
  eyebrow: string;
  title: string;
  description: string;
}) {
  return (
    <div>
      <div className="text-[10.5px] font-semibold uppercase tracking-[0.18em] text-black/35">
        {eyebrow}
      </div>
      <h3 className="mt-1 text-[14.5px] font-semibold tracking-tight text-foreground/90">
        {title}
      </h3>
      <p className="mt-1 text-[11.5px] leading-[1.6] text-muted-foreground">
        {description}
      </p>
    </div>
  );
}

function FieldGroup({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-xl border border-black/[0.06] bg-black/[0.015] px-3.5 py-3">
      <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/35">
        {label}
      </div>
      <div className="mt-2">{children}</div>
    </div>
  );
}

const editorTextareaClass =
  "min-h-[120px] rounded-lg border-black/[0.08] bg-white text-[12.5px] leading-5 text-foreground/85 focus-visible:ring-[3px] focus-visible:ring-jade/15";

export function AgentIdentitySettingsPage() {
  const [catalog, setCatalog] = useState<PromptControlCatalog | null>(null);
  const [identityPack, setLocalIdentityPack] =
    useState<IdentityCustomizationPack>({
      souls: {},
      personas: {},
    });
  const [settings, setSettings] = useState<PromptControlSettings | null>(null);
  const [selectedSoulId, setSelectedSoulId] = useState<string>("auto");
  const [selectedPersonaId, setSelectedPersonaId] = useState<string>("auto");
  const [, setLoading] = useState(true);
  const [savingDefaults, setSavingDefaults] = useState(false);
  const [savingPack, setSavingPack] = useState(false);
  const [currentSession, setCurrentSession] = useState<Session | null>(null);

  const [createPersonaOpen, setCreatePersonaOpen] = useState(false);
  const [agentNameDraft, setAgentNameDraft] = useState("");
  const [userNameDraft, setUserNameDraft] = useState("");
  const [bioDraft, setBioDraft] = useState("");
  const [missionDraft, setMissionDraft] = useState("");
  const [toneDraft, setToneDraft] = useState("");
  const [collaborationDraft, setCollaborationDraft] = useState("");
  const [outputDraft, setOutputDraft] = useState("");
  const [principlesDraft, setPrinciplesDraft] = useState("");
  const [decisionDraft, setDecisionDraft] = useState("");
  const [nonNegotiablesDraft, setNonNegotiablesDraft] = useState("");

  const load = async () => {
    setLoading(true);
    try {
      const [nextCatalog, nextPack, nextSettings] = await Promise.all([
        getIdentityCatalog(),
        getIdentityPack(),
        getIdentityDefaults(),
      ]);
      setCatalog(nextCatalog);
      setLocalIdentityPack(nextPack);
      setSettings(nextSettings);
      setSelectedSoulId(nextSettings.default_soul_id ?? "auto");
      setSelectedPersonaId(nextSettings.default_persona_id ?? "auto");
      setAgentNameDraft(nextSettings.agent_name ?? "");
      setUserNameDraft(nextSettings.user_name ?? "");
    } catch (error) {
      toast.error("读取 Agent Identity 设置失败", {
        description: String(error),
      });
    } finally {
      setLoading(false);
    }
  };

  const loadCurrentSession = async () => {
    const sessionId = readLastActiveSessionId();
    if (!sessionId) {
      setCurrentSession(null);
      return;
    }
    try {
      setCurrentSession(await getSession(sessionId));
    } catch {
      setCurrentSession(null);
    }
  };

  useEffect(() => {
    void load();
    void loadCurrentSession();
  }, []);

  useCrossWindowChange<{ sessionId: string }>(
    "cross:session-identity-changed",
    () => {
      void loadCurrentSession();
    },
  );

  const effectiveCatalog = useMemo(
    () => mergeCatalogWithPack(catalog, identityPack),
    [catalog, identityPack],
  );
  const baseSouls = catalog?.souls ?? [];
  const effectiveSouls = effectiveCatalog?.souls ?? [];
  const effectiveSoulId =
    selectedSoulId === "auto" ? (baseSouls[0]?.id ?? null) : selectedSoulId;
  const selectedSoul = useMemo<PromptControlSoulOption | null>(
    () => effectiveSouls.find((item) => item.id === effectiveSoulId) ?? null,
    [effectiveSoulId, effectiveSouls],
  );
  const selectedBaseSoul = useMemo<PromptControlSoulOption | null>(
    () => baseSouls.find((item) => item.id === effectiveSoulId) ?? null,
    [baseSouls, effectiveSoulId],
  );
  const effectivePersonas = useMemo<PromptControlPersonaOption[]>(
    () =>
      (effectiveCatalog?.personas ?? []).filter((persona) =>
        effectiveSoulId ? persona.soul_id === effectiveSoulId : true,
      ),
    [effectiveCatalog?.personas, effectiveSoulId],
  );
  const basePersonas = useMemo<PromptControlPersonaOption[]>(
    () =>
      (catalog?.personas ?? []).filter((persona) =>
        effectiveSoulId ? persona.soul_id === effectiveSoulId : true,
      ),
    [catalog?.personas, effectiveSoulId],
  );
  const selectedPersona = useMemo<PromptControlPersonaOption | null>(() => {
    const personaId = selectedPersonaId === "auto" ? null : selectedPersonaId;
    if (!personaId) return null;
    return effectivePersonas.find((item) => item.id === personaId) ?? null;
  }, [effectivePersonas, selectedPersonaId]);
  const selectedBasePersona = useMemo<PromptControlPersonaOption | null>(() => {
    const personaId = selectedPersonaId === "auto" ? null : selectedPersonaId;
    if (!personaId) return null;
    return basePersonas.find((item) => item.id === personaId) ?? null;
  }, [basePersonas, selectedPersonaId]);

  useEffect(() => {
    if (selectedPersona) {
      setBioDraft(selectedPersona.summary);
      setToneDraft(linesToTextarea(selectedPersona.tone_rules));
      setCollaborationDraft(
        linesToTextarea(selectedPersona.collaboration_rules),
      );
      setOutputDraft(linesToTextarea(selectedPersona.output_preferences));
      setMissionDraft(selectedSoul?.mission ?? "");
      setPrinciplesDraft(linesToTextarea(selectedSoul?.core_principles));
      setDecisionDraft(selectedSoul?.decision_contract ?? "");
      setNonNegotiablesDraft(linesToTextarea(selectedSoul?.non_negotiables));
      return;
    }
    setBioDraft(selectedSoul?.summary ?? "");
    setMissionDraft(selectedSoul?.mission ?? "");
    setPrinciplesDraft(linesToTextarea(selectedSoul?.core_principles));
    setDecisionDraft(selectedSoul?.decision_contract ?? "");
    setNonNegotiablesDraft(linesToTextarea(selectedSoul?.non_negotiables));
    setToneDraft("");
    setCollaborationDraft("");
    setOutputDraft("");
  }, [selectedPersona?.id, selectedSoul?.id, selectedPersona, selectedSoul]);

  const hasPersonaPackChanges = useMemo(() => {
    if (!selectedPersona || !selectedBasePersona) return false;
    return (
      maybeOverrideString(selectedBasePersona.summary, bioDraft) !== null ||
      maybeOverrideLines(selectedBasePersona.tone_rules, toneDraft) !== null ||
      maybeOverrideLines(
        selectedBasePersona.collaboration_rules,
        collaborationDraft,
      ) !== null ||
      maybeOverrideLines(
        selectedBasePersona.output_preferences,
        outputDraft,
      ) !== null
    );
  }, [
    bioDraft,
    collaborationDraft,
    outputDraft,
    selectedBasePersona,
    selectedPersona,
    toneDraft,
  ]);

  const hasSoulPackChanges = useMemo(() => {
    if (!selectedSoul || !selectedBaseSoul) return false;
    return (
      maybeOverrideString(
        selectedBaseSoul.summary,
        selectedPersona ? selectedBaseSoul.summary : bioDraft,
      ) !== null ||
      maybeOverrideString(selectedBaseSoul.mission, missionDraft) !== null ||
      maybeOverrideLines(selectedBaseSoul.core_principles, principlesDraft) !==
        null ||
      maybeOverrideString(selectedBaseSoul.decision_contract, decisionDraft) !==
        null ||
      maybeOverrideLines(
        selectedBaseSoul.non_negotiables,
        nonNegotiablesDraft,
      ) !== null
    );
  }, [
    bioDraft,
    decisionDraft,
    missionDraft,
    nonNegotiablesDraft,
    principlesDraft,
    selectedBaseSoul,
    selectedPersona,
    selectedSoul,
  ]);

  const persistDefaults = async (
    nextSoulId: string,
    nextPersonaId: string,
    overrides?: { agent_name?: string | null; user_name?: string | null },
  ) => {
    if (!settings) return;
    setSavingDefaults(true);
    try {
      const updated = await setIdentityDefaults({
        default_scenario_profile: settings.default_scenario_profile,
        prompt_diagnostics_enabled: settings.prompt_diagnostics_enabled,
        default_soul_id: nextSoulId === "auto" ? null : nextSoulId,
        default_persona_id: nextPersonaId === "auto" ? null : nextPersonaId,
        agent_name:
          overrides?.agent_name !== undefined
            ? overrides.agent_name
            : (settings.agent_name ?? null),
        user_name:
          overrides?.user_name !== undefined
            ? overrides.user_name
            : (settings.user_name ?? null),
      });
      setSettings(updated);
      setSelectedSoulId(updated.default_soul_id ?? "auto");
      setSelectedPersonaId(updated.default_persona_id ?? "auto");
      await broadcastChange("cross:prompt-control-changed", {
        agentName: updated.agent_name ?? null,
        soulId: updated.default_soul_id ?? null,
        personaId: updated.default_persona_id ?? null,
      });
      toast.success("全局默认 Identity 已应用", {
        description:
          "新会话会优先采用此 Soul / Persona；当前会话仍以 Session Override 为准。",
      });
    } catch (error) {
      toast.error("保存全局默认 Identity 失败", {
        description: String(error),
      });
    } finally {
      setSavingDefaults(false);
    }
  };

  const persistNaming = async (
    nextAgentName: string | null,
    nextUserName: string | null,
  ) => {
    await persistDefaults(selectedSoulId, selectedPersonaId, {
      agent_name: nextAgentName,
      user_name: nextUserName,
    });
  };

  const handleSoulSelect = (soulId: string) => {
    setSelectedSoulId(soulId);
    let nextPersonaId = selectedPersonaId;
    const matchingPersona = (catalog?.personas ?? []).some(
      (persona) =>
        persona.id === selectedPersonaId && persona.soul_id === soulId,
    );
    if (!matchingPersona) {
      nextPersonaId = "auto";
      setSelectedPersonaId("auto");
    }
    void persistDefaults(soulId, nextPersonaId);
  };

  const handlePersonaSelect = (persona: PromptControlPersonaOption) => {
    setSelectedSoulId(persona.soul_id);
    setSelectedPersonaId(persona.id);
    void persistDefaults(persona.soul_id, persona.id);
  };

  const handleClearPersona = () => {
    setSelectedPersonaId("auto");
    void persistDefaults(selectedSoulId, "auto");
  };

  const handleDeleteCustomPersona = async (personaId: string) => {
    if (
      !window.confirm(
        `确定删除自定义 Persona「${personaId}」吗？此操作不可撤销。`,
      )
    ) {
      return;
    }
    const nextPack: IdentityCustomizationPack = {
      souls: { ...identityPack.souls },
      personas: { ...identityPack.personas },
      custom_personas: { ...(identityPack.custom_personas ?? {}) },
    };
    delete nextPack.custom_personas![personaId];
    setSavingPack(true);
    try {
      const saved = await setIdentityPack(nextPack);
      setLocalIdentityPack(saved);
      // If the deleted persona was the active default, drop it.
      if (selectedPersonaId === personaId) {
        setSelectedPersonaId("auto");
        void persistDefaults(selectedSoulId, "auto");
      }
      // Reload catalog so the deleted persona disappears from cards.
      const nextCatalog = await getIdentityCatalog();
      setCatalog(nextCatalog);
      toast.success("自定义 Persona 已删除");
    } catch (error) {
      toast.error("删除 Persona 失败", { description: String(error) });
    } finally {
      setSavingPack(false);
    }
  };

  const handleSaveIdentityPack = async () => {
    if (!selectedSoul || !selectedBaseSoul) return;
    const nextPack: IdentityCustomizationPack = {
      souls: { ...identityPack.souls },
      personas: { ...identityPack.personas },
    };

    nextPack.souls[selectedSoul.id] = {
      summary: maybeOverrideString(
        selectedBaseSoul.summary,
        selectedPersona ? selectedSoul.summary : bioDraft,
      ),
      mission: maybeOverrideString(selectedBaseSoul.mission, missionDraft),
      core_principles: maybeOverrideLines(
        selectedBaseSoul.core_principles,
        principlesDraft,
      ),
      decision_contract: maybeOverrideString(
        selectedBaseSoul.decision_contract,
        decisionDraft,
      ),
      non_negotiables: maybeOverrideLines(
        selectedBaseSoul.non_negotiables,
        nonNegotiablesDraft,
      ),
    };

    if (
      !nextPack.souls[selectedSoul.id].summary &&
      !nextPack.souls[selectedSoul.id].mission &&
      !nextPack.souls[selectedSoul.id].core_principles &&
      !nextPack.souls[selectedSoul.id].decision_contract &&
      !nextPack.souls[selectedSoul.id].non_negotiables
    ) {
      delete nextPack.souls[selectedSoul.id];
    }

    if (selectedPersona && selectedBasePersona) {
      nextPack.personas[selectedPersona.id] = {
        summary: maybeOverrideString(selectedBasePersona.summary, bioDraft),
        tone_rules: maybeOverrideLines(
          selectedBasePersona.tone_rules,
          toneDraft,
        ),
        collaboration_rules: maybeOverrideLines(
          selectedBasePersona.collaboration_rules,
          collaborationDraft,
        ),
        output_preferences: maybeOverrideLines(
          selectedBasePersona.output_preferences,
          outputDraft,
        ),
      };
      if (
        !nextPack.personas[selectedPersona.id].summary &&
        !nextPack.personas[selectedPersona.id].tone_rules &&
        !nextPack.personas[selectedPersona.id].collaboration_rules &&
        !nextPack.personas[selectedPersona.id].output_preferences
      ) {
        delete nextPack.personas[selectedPersona.id];
      }
    }

    setSavingPack(true);
    try {
      const saved = await setIdentityPack(nextPack);
      setLocalIdentityPack(saved);
      toast.success("Identity Pack 已保存", {
        description:
          "已写入 ~/.if2ai/prompt/identity-pack.json，并用于后续 prompt 组装。",
      });
    } catch (error) {
      toast.error("保存 Identity Pack 失败", {
        description: String(error),
      });
    } finally {
      setSavingPack(false);
    }
  };

  const handleExportIdentityPack = async () => {
    try {
      const target = await save({
        title: "导出 Identity Pack",
        defaultPath: "identity-pack.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!target || typeof target !== "string") return;
      await writeTextFile(
        target,
        JSON.stringify(buildIdentityPackFileEnvelope(identityPack), null, 2),
      );
      toast.success("Identity Pack 已导出", {
        description: target,
      });
    } catch (error) {
      toast.error("导出 Identity Pack 失败", {
        description: String(error),
      });
    }
  };

  const handleImportIdentityPack = async () => {
    try {
      const selected = await open({
        title: "导入 Identity Pack",
        multiple: false,
        directory: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!selected || typeof selected !== "string") return;
      const raw = await readTextFile(selected);
      const parsed = parseIdentityPackFile(raw);
      setSavingPack(true);
      const saved = await setIdentityPack({
        souls: parsed.souls ?? {},
        personas: parsed.personas ?? {},
      });
      setLocalIdentityPack(saved);
      toast.success("Identity Pack 已导入", {
        description: selected,
      });
    } catch (error) {
      toast.error("导入 Identity Pack 失败", {
        description: String(error),
      });
    } finally {
      setSavingPack(false);
    }
  };

  const handleResetCurrentEntity = async () => {
    if (!selectedSoul) return;
    const nextPack: IdentityCustomizationPack = {
      souls: { ...identityPack.souls },
      personas: { ...identityPack.personas },
    };
    delete nextPack.souls[selectedSoul.id];
    if (selectedPersona) {
      delete nextPack.personas[selectedPersona.id];
    }

    setSavingPack(true);
    try {
      const saved = await setIdentityPack(nextPack);
      setLocalIdentityPack(saved);
      toast.success("当前 Identity override 已重置");
    } catch (error) {
      toast.error("重置 Identity override 失败", {
        description: String(error),
      });
    } finally {
      setSavingPack(false);
    }
  };

  const handleRestoreBuiltInDefaults = async () => {
    if (
      !window.confirm(
        "确定要清空所有自定义 identity pack，并恢复为内建默认吗？",
      )
    ) {
      return;
    }
    setSavingPack(true);
    try {
      const saved = await setIdentityPack({
        souls: {},
        personas: {},
      });
      setLocalIdentityPack(saved);
      toast.success("已恢复内建默认 identity", {
        description: "自定义 identity pack 已清空。",
      });
    } catch (error) {
      toast.error("恢复内建默认失败", {
        description: String(error),
      });
    } finally {
      setSavingPack(false);
    }
  };

  const handleUpdateCurrentSessionIdentity = async (
    sessionId: string,
    identity: SessionIdentityInput,
  ) => {
    const updated = await updateSessionIdentity(sessionId, identity);
    await broadcastChange("cross:session-identity-changed", {
      sessionId,
      soulId: updated.soul_id ?? null,
      personaId: updated.persona_id ?? null,
    });
    await loadCurrentSession();
  };

  const savedDefaultSoulName =
    effectiveCatalog?.souls.find((s) => s.id === settings?.default_soul_id)
      ?.name ??
    (settings?.default_soul_id ? settings.default_soul_id : "Built-in Auto");
  const savedDefaultPersonaName = settings?.default_persona_id
    ? (effectiveCatalog?.personas.find(
        (p) => p.id === settings.default_persona_id,
      )?.name ?? settings.default_persona_id)
    : "Auto Persona";

  const heroAgentName =
    settings?.agent_name && settings.agent_name.length > 0
      ? settings.agent_name
      : "If2Ai";

  return (
    <div className="flex flex-col gap-4">
      {/* ── Hero ─────────────────────────────────────────
          Two balanced columns:
            L = 文案 (eyebrow + title + 1 行精简描述)
            R = KPI 卡（3 行：name / soul / persona），独立行不挤压 */}
      <SettingsSurface className="px-6 py-5">
        <div className="grid gap-5 lg:grid-cols-[minmax(0,1.35fr)_minmax(20rem,0.85fr)] lg:items-stretch">
          <div className="flex flex-col justify-center">
            <div className="text-[10.5px] font-semibold uppercase tracking-[0.18em] text-black/35">
              Agent Identity
            </div>
            <h2 className="mt-2 text-[20px] font-semibold leading-tight tracking-tight text-foreground">
              关于 Ta
            </h2>
            <p className="mt-2 max-w-xl text-[12.5px] leading-[1.7] text-muted-foreground">
              Soul 是稳定的<span className="text-foreground/75">人格底座</span>，
              Persona 是当前对话的<span className="text-foreground/75">表达模式</span>。
              两者会实时注入到 LLM 的 system prompt，
              <span className="text-foreground/75">真的会影响每一轮回复的人格与语言风格</span>。
            </p>
            <p className="mt-1.5 text-[11px] leading-5 text-black/40">
              下方任何卡片点击即生效；当前会话以底部 Session Override 为准。
            </p>
          </div>

          <div className="relative overflow-hidden rounded-2xl border border-jade/20 bg-[linear-gradient(135deg,rgba(16,185,129,0.06),rgba(16,185,129,0.02))] p-4">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-1.5">
                <span
                  className={cn(
                    "inline-flex h-1.5 w-1.5 rounded-full bg-jade",
                    !savingDefaults && "animate-pulse",
                  )}
                />
                <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-jade">
                  {savingDefaults ? "Saving" : "Live"}
                </span>
                <span className="text-[10px] text-black/35">
                  · 当前生效的全局默认
                </span>
              </div>
              <span className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-white/85 text-jade shadow-[0_2px_6px_rgba(15,23,42,0.05)]">
                <Bot className="h-4 w-4" />
              </span>
            </div>
            <dl className="mt-3 space-y-2">
              <div className="flex items-baseline justify-between gap-3">
                <dt className="text-[10px] font-medium uppercase tracking-wider text-black/40">
                  Agent
                </dt>
                <dd className="truncate text-[14px] font-semibold tracking-tight text-foreground/90">
                  {heroAgentName}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-3 border-t border-jade/15 pt-2">
                <dt className="text-[10px] font-medium uppercase tracking-wider text-black/40">
                  Soul
                </dt>
                <dd className="truncate text-[12.5px] font-semibold text-foreground/85">
                  {savedDefaultSoulName}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-3 border-t border-jade/15 pt-2">
                <dt className="text-[10px] font-medium uppercase tracking-wider text-black/40">
                  Persona
                </dt>
                <dd className="truncate text-[12.5px] font-semibold text-foreground/85">
                  {savedDefaultPersonaName}
                </dd>
              </div>
            </dl>
          </div>
        </div>
      </SettingsSurface>

      <SettingsSurface className="px-6 py-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between lg:gap-8">
          <div className="max-w-md">
            <div className="text-[10.5px] font-semibold uppercase tracking-[0.18em] text-black/35">
              True Name
            </div>
            <h3 className="mt-1.5 text-[15px] font-semibold tracking-tight text-foreground/90">
              给 Ta 一个名字，给自己一个称呼
            </h3>
            <p className="mt-1 text-[12px] leading-[1.7] text-muted-foreground">
              名字会跨所有 Persona 恒定，作为独立 Identity block 注入到 prompt 顶部。
              <span className="text-black/40">离开输入框自动保存。</span>
            </p>
          </div>
          <div className="grid flex-1 gap-3 sm:grid-cols-2 lg:max-w-xl">
            <label className="grid gap-1.5">
              <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
                Agent
              </span>
              <input
                type="text"
                value={agentNameDraft}
                onChange={(event) => setAgentNameDraft(event.target.value)}
                onBlur={() => {
                  const next = agentNameDraft.trim();
                  const previous = settings?.agent_name ?? "";
                  if (next !== previous) {
                    void persistNaming(
                      next || null,
                      userNameDraft.trim() || null,
                    );
                  }
                }}
                placeholder="Asa / 小铭 / Hanako"
                maxLength={32}
                className="h-9 rounded-lg border border-black/[0.08] bg-white px-3 text-[13.5px] tracking-tight text-foreground/90 outline-none transition-all placeholder:text-black/25 focus:border-jade/45 focus:bg-white focus:ring-[3px] focus:ring-jade/12"
              />
            </label>
            <label className="grid gap-1.5">
              <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
                You
              </span>
              <input
                type="text"
                value={userNameDraft}
                onChange={(event) => setUserNameDraft(event.target.value)}
                onBlur={() => {
                  const next = userNameDraft.trim();
                  const previous = settings?.user_name ?? "";
                  if (next !== previous) {
                    void persistNaming(
                      agentNameDraft.trim() || null,
                      next || null,
                    );
                  }
                }}
                placeholder="RL / 老刘 / Ryan"
                maxLength={32}
                className="h-9 rounded-lg border border-black/[0.08] bg-white px-3 text-[13.5px] tracking-tight text-foreground/90 outline-none transition-all placeholder:text-black/25 focus:border-jade/45 focus:bg-white focus:ring-[3px] focus:ring-jade/12"
              />
            </label>
          </div>
        </div>
      </SettingsSurface>

      <SettingsSurface className="px-6 py-5">
        <div className="flex items-end justify-between gap-3">
          <div>
            <div className="text-[10.5px] font-semibold uppercase tracking-[0.18em] text-black/35">
              Soul
            </div>
            <h3 className="mt-1 text-[14.5px] font-semibold tracking-tight text-foreground/90">
              选择人格底座
            </h3>
          </div>
          <span className="text-[10.5px] tracking-wide text-black/35">
            点击即保存为全局默认
          </span>
        </div>
        <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {effectiveSouls.map((soul) => {
            const active = selectedSoul?.id === soul.id;
            return (
              <button
                key={soul.id}
                type="button"
                onClick={() => handleSoulSelect(soul.id)}
                className={cn(
                  "group relative rounded-2xl border bg-white px-4 py-3.5 text-left transition-all",
                  active
                    ? "border-jade/45 bg-jade/[0.04] shadow-[0_0_0_4px_rgba(16,185,129,0.06),0_4px_14px_rgba(15,23,42,0.04)]"
                    : "border-black/[0.06] hover:-translate-y-0.5 hover:border-black/[0.12] hover:shadow-[0_6px_18px_rgba(15,23,42,0.06)]",
                )}
              >
                <div className="flex items-center gap-2.5">
                  <span
                    className={cn(
                      "inline-flex h-8 w-8 items-center justify-center rounded-xl transition-colors",
                      active
                        ? "bg-jade/15 text-jade"
                        : "bg-black/[0.04] text-black/55 group-hover:bg-black/[0.06]",
                    )}
                  >
                    <Bot className="h-4 w-4" />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-semibold tracking-tight text-foreground/88">
                      {soul.name}
                    </div>
                    <div className="text-[10.5px] tracking-wide text-black/40">
                      version {soul.version}
                    </div>
                  </div>
                  {active ? (
                    <Check className="h-3.5 w-3.5 shrink-0 text-jade" />
                  ) : null}
                </div>
                <div className="mt-2.5 line-clamp-2 text-[11.5px] leading-[1.55] text-muted-foreground">
                  {soul.summary}
                </div>
              </button>
            );
          })}
        </div>

        <div className="mt-7 flex items-end justify-between gap-3">
          <div>
            <div className="text-[10.5px] font-semibold uppercase tracking-[0.18em] text-black/35">
              Persona
            </div>
            <h3 className="mt-1 text-[14.5px] font-semibold tracking-tight text-foreground/90">
              选择表达模式
            </h3>
            <p className="mt-0.5 text-[11px] leading-4 text-black/40">
              切的是「状态」，不是「身份」—— 同一个 Soul 下挂多个 Persona。
            </p>
          </div>
          <div className="flex items-center gap-3">
            {selectedPersonaId !== "auto" ? (
              <button
                type="button"
                className="text-[10.5px] font-medium text-black/45 underline-offset-2 transition-colors hover:text-foreground hover:underline disabled:opacity-40"
                onClick={handleClearPersona}
                disabled={savingDefaults}
              >
                清除 · 跟随默认
              </button>
            ) : null}
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-7 rounded-lg border-jade/40 bg-jade/[0.06] px-2.5 text-[11.5px] font-semibold text-jade hover:bg-jade/[0.12] hover:text-jade"
              onClick={() => setCreatePersonaOpen(true)}
              disabled={!catalog}
            >
              <Plus className="mr-1 h-3 w-3" strokeWidth={2.5} />
              新建 Persona
            </Button>
          </div>
        </div>
        <div className="mt-4 grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {effectivePersonas.map((persona, index) => {
            const active = selectedPersona?.id === persona.id;
            // Custom persona = lives in the pack's custom_personas map.
            // Built-in personas can't be deleted from this UI.
            const isCustom = Boolean(
              identityPack.custom_personas?.[persona.id],
            );
            // 把「Execution Partner / 执行伙伴」拆成中英两行，让卡片更工整
            const [primaryName, ...rest] = persona.name.split(" / ");
            const secondaryName = rest.join(" / ").trim();
            // 简介按 " / " 分中英；只显示中文部分（英文降为 hover 时 title）
            const summarySplit = persona.summary.split(" / ");
            const primarySummary = summarySplit[0]?.trim() ?? "";

            return (
              <div
                key={persona.id}
                role="button"
                tabIndex={0}
                onClick={() => handlePersonaSelect(persona)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    handlePersonaSelect(persona);
                  }
                }}
                title={persona.summary}
                className={cn(
                  "group relative flex cursor-pointer flex-col items-center overflow-hidden rounded-2xl border bg-white px-5 pb-5 pt-6 text-center outline-none transition-all duration-300 focus-visible:ring-[3px] focus-visible:ring-jade/20",
                  active
                    ? "border-jade/40 shadow-[0_0_0_4px_rgba(16,185,129,0.07),0_10px_28px_rgba(15,23,42,0.07)]"
                    : "border-black/[0.06] hover:-translate-y-0.5 hover:border-black/[0.1] hover:shadow-[0_10px_24px_rgba(15,23,42,0.06)]",
                )}
              >
                {/* Delete affordance (custom only, fade in on hover) */}
                {isCustom ? (
                  <button
                    type="button"
                    onClick={(event) => {
                      event.stopPropagation();
                      void handleDeleteCustomPersona(persona.id);
                    }}
                    title="删除自定义 Persona"
                    aria-label="删除自定义 Persona"
                    className="absolute left-2 top-2 z-10 inline-flex h-6 w-6 items-center justify-center rounded-md text-black/35 opacity-0 transition-all hover:bg-red-50 hover:text-red-600 group-hover:opacity-100 focus:opacity-100"
                  >
                    <Trash2 className="h-3 w-3" />
                  </button>
                ) : null}
                {/* Custom badge */}
                {isCustom ? (
                  <span className="absolute right-2 top-2 rounded-md bg-amber-50/90 px-1.5 py-0.5 text-[9.5px] font-semibold uppercase tracking-[0.14em] text-amber-700">
                    Custom
                  </span>
                ) : null}
                {/* Active indicator: jade halo behind avatar (subtle) */}
                {active ? (
                  <span
                    aria-hidden
                    className="pointer-events-none absolute left-1/2 top-3 h-32 w-32 -translate-x-1/2 rounded-full bg-[radial-gradient(circle,rgba(16,185,129,0.10),transparent_65%)]"
                  />
                ) : null}

                <div
                  className={cn(
                    "relative transition-transform duration-300",
                    active ? "scale-[1.03]" : "group-hover:scale-[1.04]",
                  )}
                >
                  <PersonaPortrait
                    label={persona.name}
                    index={index}
                    personaId={persona.id}
                    avatarId={persona.avatar_id}
                    size={108}
                  />
                  {active ? (
                    <span className="absolute -right-1 -top-1 inline-flex h-6 w-6 items-center justify-center rounded-full bg-jade text-white shadow-[0_3px_8px_rgba(16,185,129,0.35)] ring-2 ring-white">
                      <Check className="h-3.5 w-3.5" strokeWidth={3} />
                    </span>
                  ) : null}
                </div>

                <div className="mt-4 w-full min-w-0">
                  <div className="truncate text-[15px] font-semibold leading-tight tracking-tight text-foreground/90">
                    {primaryName}
                  </div>
                  {secondaryName ? (
                    <div className="mt-0.5 truncate text-[10.5px] font-medium uppercase tracking-[0.14em] text-black/35">
                      {secondaryName}
                    </div>
                  ) : null}
                  <div className="mt-2.5 line-clamp-2 text-[11.5px] leading-[1.6] text-muted-foreground">
                    {primarySummary}
                  </div>
                </div>

                {/* Bottom indicator bar (jade, like macOS dock active dot) */}
                <span
                  aria-hidden
                  className={cn(
                    "absolute bottom-0 left-1/2 h-[3px] w-10 -translate-x-1/2 rounded-t-full transition-all duration-300",
                    active ? "bg-jade" : "bg-transparent",
                  )}
                />
              </div>
            );
          })}
        </div>
      </SettingsSurface>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.45fr)_minmax(20rem,0.9fr)]">
        <SettingsSurface className="px-6 py-5">
          <div className="flex flex-col gap-5">
            <div>
              <SectionHeader
                eyebrow="身份简介"
                title={
                  selectedPersona
                    ? `${selectedPersona.name} 简介`
                    : `${selectedSoul?.name ?? "Identity"} 简介`
                }
                description="简短描述助手是谁、擅长什么、协作节奏。保存后会写入自定义 identity pack。"
              />
              <Textarea
                value={bioDraft}
                onChange={(event) => setBioDraft(event.target.value)}
                className="mt-2.5 min-h-[140px] rounded-xl border-black/[0.08] bg-white px-3.5 py-3 text-[13px] leading-6 text-foreground/88 focus-visible:ring-[3px] focus-visible:ring-jade/15"
                placeholder="例如：RL 的个人助手。擅长在结构化分析和温和陪伴之间保持平衡。"
              />
            </div>

            <div>
              <SectionHeader
                eyebrow="意识"
                title="可编辑 Identity Pack"
                description={
                  selectedPersona
                    ? "Persona 控制语气、协作方式与输出偏好。编辑会持久化到 ~/.if2ai/prompt/identity-pack.json。"
                    : "未选择 Persona 时，Soul 提供底层的行为约束与决策契约。"
                }
              />

              {selectedPersona ? (
                <div className="mt-2.5 grid gap-2">
                  <FieldGroup label="Tone Rules">
                    <Textarea
                      value={toneDraft}
                      onChange={(event) => setToneDraft(event.target.value)}
                      className={editorTextareaClass}
                      placeholder={"- 保持友善、真诚的交流风格\n- 解释概念时从底层原理出发"}
                    />
                  </FieldGroup>
                  <FieldGroup label="Collaboration Rules">
                    <Textarea
                      value={collaborationDraft}
                      onChange={(event) => setCollaborationDraft(event.target.value)}
                      className={editorTextareaClass}
                      placeholder={"- 善于举一反三，用类比帮助理解\n- 在存在风险时明确指出边界和假设"}
                    />
                  </FieldGroup>
                  <FieldGroup label="Output Preferences">
                    <Textarea
                      value={outputDraft}
                      onChange={(event) => setOutputDraft(event.target.value)}
                      className={editorTextareaClass}
                      placeholder={"- 先给结论，再给理由\n- 复杂问题优先分层拆解"}
                    />
                  </FieldGroup>
                </div>
              ) : (
                <div className="mt-2.5 grid gap-2">
                  <FieldGroup label="Core Principles">
                    <Textarea
                      value={principlesDraft}
                      onChange={(event) => setPrinciplesDraft(event.target.value)}
                      className={editorTextareaClass}
                      placeholder={"- 优先真实和可验证的帮助\n- 在推进执行和风险提示之间保持平衡"}
                    />
                  </FieldGroup>
                  <FieldGroup label="Decision Contract">
                    <Textarea
                      value={decisionDraft}
                      onChange={(event) => setDecisionDraft(event.target.value)}
                      className={editorTextareaClass}
                      placeholder="例如：保持用户主导权，验证不稳定事实，并明确说明假设。"
                    />
                  </FieldGroup>
                  <FieldGroup label="Non-negotiables">
                    <Textarea
                      value={nonNegotiablesDraft}
                      onChange={(event) => setNonNegotiablesDraft(event.target.value)}
                      className={editorTextareaClass}
                      placeholder={"- 不隐藏不确定性\n- 不绕过系统安全和权限边界"}
                    />
                  </FieldGroup>
                </div>
              )}
            </div>
          </div>
        </SettingsSurface>

        <div className="flex flex-col gap-4">
          <SettingsSurface className="px-6 py-5">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-1.5 text-[10.5px] font-semibold uppercase tracking-[0.18em] text-black/35">
                <Sparkles className="h-3 w-3 text-jade" />
                Snapshot
              </div>
              <span
                className={cn(
                  "inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[9.5px] font-semibold uppercase tracking-[0.14em] text-jade",
                )}
              >
                <span
                  className={cn(
                    "h-1.5 w-1.5 rounded-full bg-jade",
                    !savingDefaults && "animate-pulse",
                  )}
                />
                {savingDefaults ? "Saving" : "Live"}
              </span>
            </div>
            <div className="mt-3 flex flex-col gap-2">
              <div className="rounded-lg border border-jade/25 bg-jade/[0.04] px-3 py-2">
                <div className="text-[10px] font-semibold uppercase tracking-widest text-jade/85">
                  Active Global Default
                </div>
                <div className="mt-0.5 text-[12.5px] font-semibold text-foreground/85">
                  {savedDefaultSoulName} · {savedDefaultPersonaName}
                </div>
                <div className="mt-0.5 text-[10.5px] text-muted-foreground">
                  新会话默认采用此组合（无 session override 时）。
                </div>
              </div>
              <div className="rounded-lg border border-dashed border-black/[0.08] bg-black/[0.02] px-3 py-2">
                <div className="text-[10.5px] font-medium text-black/40">
                  当前正在编辑
                </div>
                <div className="mt-0.5 text-[12.5px] font-semibold text-foreground/85">
                  {selectedSoul?.name ?? "Built-in Auto"}
                  {selectedPersona ? ` · ${selectedPersona.name}` : " · Auto Persona"}
                </div>
                <div className="mt-0.5 text-[10.5px] text-muted-foreground">
                  下方表单修改的是这一身份的 identity pack 内容。
                </div>
              </div>
              <div className="rounded-lg border border-black/[0.06] bg-black/[0.015] px-3 py-2">
                <div className="flex items-center justify-between gap-2">
                  <div className="text-[10.5px] font-medium text-black/40">
                    Identity Pack Path
                  </div>
                  <button
                    type="button"
                    className="text-[10.5px] font-medium text-jade hover:underline"
                    onClick={() => {
                      void navigator.clipboard
                        .writeText("~/.if2ai/prompt/identity-pack.json")
                        .then(() =>
                          toast.success("路径已复制到剪贴板", {
                            description:
                              "粘贴到 Finder「前往 → 前往文件夹」即可定位。",
                          }),
                        )
                        .catch(() =>
                          toast.error("复制路径失败"),
                        );
                    }}
                  >
                    复制路径
                  </button>
                </div>
                <div className="mt-0.5 font-mono text-[11px] text-foreground/80">
                  ~/.if2ai/prompt/identity-pack.json
                </div>
                <div className="mt-1 text-[10.5px] leading-4 text-muted-foreground">
                  仅当点击「保存 Identity Pack 修改」并产生过有效 override 时，文件才会被创建。空 override 不会写盘（保持本地目录干净）。
                </div>
              </div>
            </div>
            <div className="mt-3 grid gap-1.5">
              <Button
                type="button"
                size="sm"
                className="h-8 rounded-lg text-[11.5px]"
                disabled={savingPack || !(hasPersonaPackChanges || hasSoulPackChanges)}
                onClick={() => void handleSaveIdentityPack()}
              >
                {savingPack ? "保存中..." : "保存 Identity Pack 修改"}
              </Button>
              <div className="grid grid-cols-2 gap-1.5">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 rounded-lg border-black/[0.09] bg-black/[0.025] text-[11.5px] hover:bg-black/[0.05]"
                  disabled={savingPack}
                  onClick={() => void handleImportIdentityPack()}
                >
                  导入 Pack
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 rounded-lg border-black/[0.09] bg-black/[0.025] text-[11.5px] hover:bg-black/[0.05]"
                  disabled={savingPack}
                  onClick={() => void handleExportIdentityPack()}
                >
                  导出 Pack
                </Button>
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-8 rounded-lg border-black/[0.09] bg-black/[0.025] text-[11.5px] hover:bg-black/[0.05]"
                disabled={savingPack || (!selectedPersona && !selectedSoul)}
                onClick={() => void handleResetCurrentEntity()}
              >
                <RotateCcw className="mr-1.5 h-3 w-3" />
                重置当前身份的 Pack 修改
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-8 rounded-lg border-black/[0.09] bg-black/[0.025] text-[11.5px] hover:bg-black/[0.05]"
                disabled={savingPack}
                onClick={() => void handleRestoreBuiltInDefaults()}
              >
                恢复内建默认（清空所有 Pack 自定义）
              </Button>
            </div>
          </SettingsSurface>

          <SettingsSurface className="px-5 py-4">
            <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/35">
              Soul Mission
            </div>
            <Textarea
              value={missionDraft}
              onChange={(event) => setMissionDraft(event.target.value)}
              className="mt-2 min-h-[120px] rounded-lg border-black/[0.08] bg-white text-[12.5px] leading-5"
              placeholder="这个 Soul 想长期帮助用户完成什么样的工作。"
            />
          </SettingsSurface>

          {selectedPersona ? (
            <SettingsSurface className="px-5 py-4">
              <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/35">
                Soul Guardrails
              </div>
              <Textarea
                value={nonNegotiablesDraft}
                onChange={(event) => setNonNegotiablesDraft(event.target.value)}
                className="mt-2 min-h-[120px] rounded-lg border-black/[0.08] bg-white text-[12.5px] leading-5"
                placeholder="- 这里是底层不可违反的边界"
              />
            </SettingsSurface>
          ) : null}
        </div>
      </div>

      <SettingsSurface className="px-6 py-5">
        <div className="mb-4 flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="text-[10.5px] font-semibold uppercase tracking-[0.18em] text-black/35">
              Session Override
            </div>
            <h3 className="mt-1 text-[14.5px] font-semibold tracking-tight text-foreground/90">
              当前会话临时切换
            </h3>
            <p className="mt-1 text-[11.5px] leading-[1.6] text-muted-foreground">
              <span className="font-semibold text-foreground/75">优先级最高</span>：
              session 存在 override 时会忽略全局默认；只影响最近活跃会话，不改写新会话默认。
            </p>
          </div>
          <div className="inline-flex shrink-0 items-center gap-1 rounded-md bg-amber-50/80 px-1.5 py-0.5 text-[10px] font-medium tracking-wide text-amber-700">
            <Shield className="h-3 w-3" />
            Session only
          </div>
        </div>

        <div className="mb-4 grid gap-2.5 sm:grid-cols-2">
          <div className="rounded-xl border border-jade/20 bg-jade/[0.035] px-3.5 py-2.5">
            <div className="text-[10px] font-semibold uppercase tracking-[0.16em] text-jade/85">
              Global Default
            </div>
            <div className="mt-1 truncate text-[12.5px] font-semibold text-foreground/88">
              {savedDefaultSoulName}
              <span className="text-black/30"> · </span>
              {savedDefaultPersonaName}
            </div>
          </div>
          <div className="rounded-xl border border-amber-200/50 bg-amber-50/30 px-3.5 py-2.5">
            <div className="text-[10px] font-semibold uppercase tracking-[0.16em] text-amber-700/85">
              Session 实际生效
            </div>
            <div className="mt-1 truncate text-[12.5px] font-semibold text-foreground/88">
              {currentSession
                ? `${
                    currentSession.soul_id
                      ? (effectiveCatalog?.souls.find((s) => s.id === currentSession.soul_id)?.name ?? currentSession.soul_id)
                      : `${savedDefaultSoulName}`
                  } · ${
                    currentSession.persona_id
                      ? (effectiveCatalog?.personas.find((p) => p.id === currentSession.persona_id)?.name ?? currentSession.persona_id)
                      : `${savedDefaultPersonaName}`
                  }${
                    !currentSession.soul_id && !currentSession.persona_id
                      ? "  (跟随默认)"
                      : ""
                  }`
                : "未检测到活跃会话"}
            </div>
          </div>
        </div>

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
          catalog={effectiveCatalog}
          onUpdate={async (sessionId, identity) => {
            await handleUpdateCurrentSessionIdentity(sessionId, identity);
          }}
        />
      </SettingsSurface>

      <CreatePersonaDialog
        open={createPersonaOpen}
        onOpenChange={setCreatePersonaOpen}
        catalog={catalog}
        defaultSoulId={selectedSoulId === "auto" ? null : selectedSoulId}
        currentPack={identityPack}
        onSaved={async (saved) => {
          setLocalIdentityPack(saved);
          // Reload catalog so the new persona shows up in the cards.
          try {
            const nextCatalog = await getIdentityCatalog();
            setCatalog(nextCatalog);
          } catch {
            // best-effort; the page will catch up on next manual reload
          }
        }}
      />
    </div>
  );
}
