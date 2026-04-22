import { useEffect, useMemo, useState } from "react";
import { Bot, Check, Sparkles, Wand2 } from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  PERSONA_AVATAR_LIBRARY,
  PERSONA_AVATAR_BY_ID,
} from "@/lib/persona-avatars";
import { cn } from "@/lib/utils";
import {
  setIdentityPack,
  type CustomPersonaDefinition,
  type IdentityCustomizationPack,
  type PromptControlCatalog,
} from "@/api/identity";

interface CreatePersonaDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Effective catalog (built-in + already-saved custom personas). */
  catalog: PromptControlCatalog | null;
  /** Soul that the new persona attaches to (typically the active soul). */
  defaultSoulId: string | null;
  /** Current pack — we patch its `custom_personas` and persist back. */
  currentPack: IdentityCustomizationPack;
  /** Called with the saved pack so the parent can refresh local state. */
  onSaved: (savedPack: IdentityCustomizationPack) => void | Promise<void>;
  /**
   * Edit mode: when set, the dialog pre-fills with this persona's data
   * and the save action **patches the same id** instead of creating a
   * new entry. The id field becomes read-only because changing it would
   * effectively orphan the entry (referrers like default_persona_id /
   * session.persona_id would still point at the old id).
   */
  editingPersonaId?: string | null;
}

// ── Slug + collision-safe id generation ─────────────────────────────

/** Convert "Quiet Mind / 静心 ✨" → "quiet-mind". */
function slugify(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[\u4e00-\u9fa5]+/g, "") // strip CJK (slugify only Latin part)
    .replace(/[^\p{Letter}\p{Number}\s-]/gu, "")
    .replace(/\s+/g, "-")
    .replace(/-{2,}/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 32);
}

/** Generate a unique persona id given a name and the set of taken ids.
 *  Falls back to a Math.random suffix if the name is purely CJK. */
function generateUniqueId(name: string, takenIds: Set<string>): string {
  const base = slugify(name) || `persona-${Math.random().toString(36).slice(2, 8)}`;
  if (!takenIds.has(base)) return base;
  for (let i = 2; i < 999; i += 1) {
    const candidate = `${base}-${i}`;
    if (!takenIds.has(candidate)) return candidate;
  }
  return `${base}-${Date.now()}`;
}

// ── Local template-based "smart expansion" ──────────────────────────

/** Heuristic: split lines, drop empty / leading bullets. */
function linesFromText(value: string): string[] {
  return value
    .split("\n")
    .map((line) => line.replace(/^\s*[-*]\s*/, "").trim())
    .filter(Boolean);
}

/**
 * Map common Chinese / English vibe keywords to bilingual rule snippets.
 * Detection is substring-based and case-insensitive — order in this map
 * also defines the rule rendering order in the output, so put the more
 * "personality-defining" traits first.
 */
const VIBE_LIBRARY: Array<{
  keys: string[];
  tone: string;
  collab: string;
  output: string;
}> = [
  {
    keys: ["温柔", "温暖", "tender", "warm", "gentle"],
    tone: "保持温暖、真诚的语气，关心对方但不刻意 / Warm, sincere, attentive without being saccharine.",
    collab: "在对方表达情绪时先承接，再给方案 / Acknowledge emotion before jumping to solutions.",
    output: "用柔和的句式收尾，不强势收口 / Close with soft phrasing; never dictate.",
  },
  {
    keys: ["共情", "倾听", "empath", "listen"],
    tone: "倾听优先，回应紧扣对方真正在意的点 / Listen first; respond to what the user actually cares about.",
    collab: "用提问澄清需求，而非直接断言 / Clarify with questions before asserting.",
    output: "先复述你听到的，再给出建议 / Reflect what you heard, then suggest.",
  },
  {
    keys: ["克制", "冷静", "理性", "calm", "rational", "restrained"],
    tone: "克制、精准、不废话；像值得信赖的顾问 / Restrained, precise, no filler — like a trusted advisor.",
    collab: "在面对方案 / 想法时，先拆解前提，再评价结论 / Deconstruct premises before assessing conclusions.",
    output: "结论先行，理由次之，证据在最后 / Recommendation first, rationale second, evidence last.",
  },
  {
    keys: ["数据", "分析", "data", "analyt", "metric"],
    tone: "用数据 / 量化表达替代模糊形容 / Replace vague adjectives with data and quantification.",
    collab: "提建议时附上可证伪的指标 / Pair every recommendation with a falsifiable metric.",
    output: "结构化输出：数据 + 解读 + 行动项 / Structured output: data → interpretation → action items.",
  },
  {
    keys: ["执行", "推力", "momentum", "action", "ship"],
    tone: "保持具体、面向实现；少抽象、多动作 / Stay concrete and implementation-focused.",
    collab: "倾向「先做下一个合理步骤」而不是过度规划 / Bias toward the next reasonable step over over-planning.",
    output: "执行过程中给短进度更新，结尾给 outcome 摘要 / Short progress updates; end with outcome summary + verification.",
  },
  {
    keys: ["创意", "想象", "灵感", "creative", "imagin", "muse"],
    tone: "保持开放、跳跃，鼓励对方放飞 / Stay open and associative; invite the user to play.",
    collab: "给多种方向供选择，而不是单一最优解 / Offer multiple directions, not a single \"best\" answer.",
    output: "用类比 / 故事 / 对比让抽象想法落地 / Ground abstract ideas with analogy, story, or contrast.",
  },
  {
    keys: ["严谨", "rigor", "precise", "rigorous"],
    tone: "用精确的术语，避免歧义 / Use precise terminology; avoid ambiguity.",
    collab: "对每个假设打上「待验证」标签 / Tag every assumption as \"to verify\".",
    output: "区分「事实 / 推理 / 推测」三类 / Distinguish facts vs inference vs speculation.",
  },
  {
    keys: ["陪伴", "耐心", "companion", "patient"],
    tone: "节奏放慢，给对方足够的时间消化 / Slow the pace; give the user time to process.",
    collab: "不打断，鼓励对方说完再回应 / Never interrupt; let the user finish before responding.",
    output: "用短段落 + 停顿，便于跟读 / Short paragraphs with pauses for easy reading.",
  },
  {
    keys: ["直接", "果断", "direct", "decisive", "blunt"],
    tone: "直接表达观点，不绕弯子 / State your view directly; no detours.",
    collab: "在 tradeoff 不明显时主动拍板，不把决策推回去 / Make the call when tradeoffs are clear; don't punt back.",
    output: "用一句话给结论，再用 2-3 个点支撑 / One-line conclusion, 2-3 supporting bullets.",
  },
  {
    keys: ["架构", "tradeoff", "architect", "system"],
    tone: "用冷静、自信的架构语言，避免「绝对」「一定」/ Calm, confident architectural language; avoid absolutes.",
    collab: "改动有隐性风险时主动给分阶段 rollout / Propose phased rollout when change carries hidden risk.",
    output: "权衡按主题归组；3 个关键 tradeoff 胜过 12 条碎片 / Group tradeoffs by theme; 3 key ones beat 12 fragments.",
  },
];

const FALLBACK_TONE = [
  "用与名字气质一致的语气与对方互动 / Adopt a tone that fits the persona's name and vibe.",
  "少用破折号（——、-）；不用「总的来说」「希望对你有帮助」收尾 / Avoid em-dashes and filler closing phrases.",
];
const FALLBACK_COLLAB = [
  "先理解需求和上下文，再给出方案 / Understand the need and context before proposing solutions.",
  "遇到不确定的事实，明确说明并寻求确认 / Surface uncertainty explicitly and ask for confirmation.",
];
const FALLBACK_OUTPUT = [
  "结论先行，再给理由 / Lead with the conclusion, then the rationale.",
  "复杂内容用结构化清单呈现 / Use structured lists for complex content.",
];

/**
 * Generate a draft summary + 3 rule lists from a short vibe string.
 * Pure local heuristic — no network, no LLM. Designed to give the user
 * a high-quality starting skeleton they can edit, not a finished pack.
 */
function expandFromVibe(opts: {
  name: string;
  vibe: string;
  soulName: string;
}): {
  summary: string;
  tone: string[];
  collab: string[];
  output: string[];
} {
  const vibeLower = opts.vibe.toLowerCase().trim();
  const matched = VIBE_LIBRARY.filter((entry) =>
    entry.keys.some((key) => vibeLower.includes(key.toLowerCase())),
  );

  // Dedup by stable order: matches first, then fallbacks.
  const tone: string[] = [];
  const collab: string[] = [];
  const output: string[] = [];
  for (const entry of matched) {
    tone.push(entry.tone);
    collab.push(entry.collab);
    output.push(entry.output);
  }
  for (const fallback of FALLBACK_TONE) {
    if (tone.length < 3) tone.push(fallback);
  }
  for (const fallback of FALLBACK_COLLAB) {
    if (collab.length < 3) collab.push(fallback);
  }
  for (const fallback of FALLBACK_OUTPUT) {
    if (output.length < 3) output.push(fallback);
  }

  const summary =
    opts.vibe.trim().length > 0
      ? `${opts.vibe.trim()} —— 挂在「${opts.soulName}」底座上的 Persona。`
      : `「${opts.soulName}」底座下的一个 Persona。`;

  return { summary, tone, collab, output };
}

// ── Component ───────────────────────────────────────────────────────

export function CreatePersonaDialog({
  open,
  onOpenChange,
  catalog,
  defaultSoulId,
  currentPack,
  onSaved,
  editingPersonaId,
}: CreatePersonaDialogProps) {
  const isEditMode = Boolean(editingPersonaId);
  const editingPersona = editingPersonaId
    ? currentPack.custom_personas?.[editingPersonaId]
    : null;

  const [name, setName] = useState("");
  const [soulId, setSoulId] = useState<string>("");
  const [vibe, setVibe] = useState("");
  const [summary, setSummary] = useState("");
  const [avatarId, setAvatarId] = useState<string | null>(null);
  const [toneText, setToneText] = useState("");
  const [collabText, setCollabText] = useState("");
  const [outputText, setOutputText] = useState("");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [saving, setSaving] = useState(false);

  // Reset form on open. Two seed strategies depending on mode:
  //   - edit: pre-fill from `editingPersona`, expand advanced section
  //     (the user came here to edit rules, hide-by-default would just
  //     force an extra click), keep vibe blank since it's an input
  //     to the smart-expand button, not a stored field.
  //   - create: blank slate with defaults.
  useEffect(() => {
    if (!open) return;
    if (isEditMode && editingPersona) {
      setName(editingPersona.name);
      setSoulId(editingPersona.soul_id);
      setSummary(editingPersona.summary);
      setAvatarId(editingPersona.avatar_id ?? null);
      setToneText((editingPersona.tone_rules ?? []).join("\n"));
      setCollabText((editingPersona.collaboration_rules ?? []).join("\n"));
      setOutputText((editingPersona.output_preferences ?? []).join("\n"));
      setVibe("");
      setAdvancedOpen(true);
      return;
    }
    setName("");
    setVibe("");
    setSummary("");
    setSoulId(
      defaultSoulId && defaultSoulId !== "auto"
        ? defaultSoulId
        : (catalog?.souls[0]?.id ?? ""),
    );
    setAvatarId(null);
    setToneText("");
    setCollabText("");
    setOutputText("");
    setAdvancedOpen(false);
  }, [open, defaultSoulId, catalog, isEditMode, editingPersona]);

  // Existing persona ids (built-in + custom) — used both for collision
  // avoidance during id auto-generation and to pre-fade in-use avatars.
  // In edit mode we exclude the persona being edited from the taken set
  // so the previewId calculation can stay deterministic at the same id.
  const takenIds = useMemo(() => {
    const set = new Set<string>();
    catalog?.personas.forEach((persona) => set.add(persona.id));
    Object.keys(currentPack.custom_personas ?? {}).forEach((key) => {
      if (key !== editingPersonaId) set.add(key);
    });
    return set;
  }, [catalog, currentPack, editingPersonaId]);

  const avatarsInUse = useMemo(() => {
    const set = new Set<string>();
    Object.values(currentPack.custom_personas ?? {}).forEach((persona) => {
      if (persona.avatar_id) set.add(persona.avatar_id);
    });
    return set;
  }, [currentPack]);

  const trimmedName = name.trim();
  const trimmedSummary = summary.trim();

  // In edit mode the id is locked to the original to avoid orphaning
  // any references (default_persona_id / session.persona_id). In create
  // mode it auto-derives from the name with collision-safe suffixing.
  const previewId = useMemo(
    () =>
      isEditMode && editingPersonaId
        ? editingPersonaId
        : generateUniqueId(trimmedName, takenIds),
    [trimmedName, takenIds, isEditMode, editingPersonaId],
  );

  const soulName = useMemo(
    () =>
      catalog?.souls.find((soul) => soul.id === soulId)?.name ??
      "Built-in Soul",
    [catalog, soulId],
  );

  const handleAutoExpand = () => {
    if (!trimmedName) {
      toast.error("先给 Ta 起个名字");
      return;
    }
    const expanded = expandFromVibe({
      name: trimmedName,
      vibe,
      soulName,
    });
    if (!summary.trim()) setSummary(expanded.summary);
    setToneText(expanded.tone.join("\n"));
    setCollabText(expanded.collab.join("\n"));
    setOutputText(expanded.output.join("\n"));
    setAdvancedOpen(true);
    toast.success("已自动扩写", {
      description: "可继续在「行为规则」里微调，或直接保存。",
    });
  };

  const canSave =
    trimmedName.length > 0 &&
    soulId.length > 0 &&
    trimmedSummary.length > 0 &&
    !saving;

  const handleSave = async () => {
    if (!canSave) return;
    setSaving(true);
    try {
      const newPersona: CustomPersonaDefinition = {
        soul_id: soulId,
        name: trimmedName,
        summary: trimmedSummary,
        avatar_id: avatarId ?? null,
        tone_rules: linesFromText(toneText),
        collaboration_rules: linesFromText(collabText),
        output_preferences: linesFromText(outputText),
      };
      const nextPack: IdentityCustomizationPack = {
        souls: { ...currentPack.souls },
        personas: { ...currentPack.personas },
        custom_personas: {
          ...(currentPack.custom_personas ?? {}),
          [previewId]: newPersona,
        },
      };
      const saved = await setIdentityPack(nextPack);
      toast.success(isEditMode ? "Persona 已更新" : "自定义 Persona 已创建", {
        description: isEditMode
          ? `${trimmedName} 的修改已保存，下一轮回复立即生效。`
          : `${trimmedName} 现在可在 Persona 列表里点击切换。`,
      });
      await onSaved(saved);
      onOpenChange(false);
    } catch (error) {
      toast.error(isEditMode ? "更新 Persona 失败" : "创建 Persona 失败", {
        description: String(error),
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          // Override the base `grid gap-4 p-6` with a flex column so we
          // can give the middle section a scroll container while keeping
          // header + footer pinned. `max-h-[90vh]` caps the dialog so it
          // never overflows the viewport (which used to push the save
          // button below the visible area when rules were expanded).
          "flex max-h-[90vh] w-[min(94vw,42rem)] max-w-[42rem] flex-col gap-0 overflow-hidden p-0",
        )}
      >
        {/* Header — pinned, doesn't scroll */}
        <DialogHeader className="shrink-0 border-b border-black/[0.06] px-6 py-4">
          <DialogTitle className="text-[16px] font-semibold tracking-tight">
            {isEditMode ? "编辑 Persona" : "新建 Persona"}
          </DialogTitle>
          <DialogDescription className="text-[12px] leading-[1.6]">
            {isEditMode ? (
              <>
                修改名字、Soul、头像或行为规则；保存后<span className="font-medium text-foreground/70">下一轮回复立即生效</span>。
                ID 在编辑模式下不可改，避免影响已有引用。
              </>
            ) : (
              <>
                填名字 + 一句话特征，<span className="font-medium text-foreground/70">点「智能扩写」</span>
                自动生成完整人格规则，再选个头像即可。所有内容保存后随时可在「可编辑 Identity Pack」继续微调。
              </>
            )}
          </DialogDescription>
        </DialogHeader>

        {/* Scrollable body */}
        <div className="grid min-h-0 flex-1 gap-4 overflow-y-auto px-6 py-5">
          {/* Name + Soul */}
          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_180px]">
            <label className="grid gap-1.5">
              <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
                Name · 名字
              </span>
              <input
                type="text"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="Quiet Mind / 静心"
                maxLength={64}
                className="h-9 rounded-lg border border-black/[0.08] bg-white px-3 text-[13.5px] tracking-tight text-foreground/90 outline-none transition-all placeholder:text-black/25 focus:border-jade/45 focus:ring-[3px] focus:ring-jade/12"
              />
              {trimmedName ? (
                <span className="text-[10.5px] tracking-wide text-black/35">
                  {isEditMode ? "ID（不可改）" : "自动 ID"}:{" "}
                  <code className="rounded bg-black/[0.04] px-1 py-px font-mono text-[10.5px] text-foreground/65">
                    {previewId}
                  </code>
                </span>
              ) : null}
            </label>
            <label className="grid gap-1.5">
              <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
                Soul · 挂在哪个底座
              </span>
              <select
                value={soulId}
                onChange={(event) => setSoulId(event.target.value)}
                className="h-9 rounded-lg border border-black/[0.08] bg-white px-2 text-[12.5px] text-foreground/90 outline-none transition-all focus:border-jade/45 focus:ring-[3px] focus:ring-jade/12"
              >
                {(catalog?.souls ?? []).map((soul) => (
                  <option key={soul.id} value={soul.id}>
                    {soul.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {/* Vibe + Auto expand */}
          <div className="grid gap-1.5">
            <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
              Vibe · 角色特征 <span className="text-black/30 normal-case tracking-normal">（一句话或几个关键词都行）</span>
            </span>
            <div className="flex gap-2">
              <input
                type="text"
                value={vibe}
                onChange={(event) => setVibe(event.target.value)}
                placeholder="温柔 / 共情 / 数据导向 · 或：温柔的数据分析师"
                maxLength={140}
                className="h-9 flex-1 rounded-lg border border-black/[0.08] bg-white px-3 text-[13px] tracking-tight text-foreground/90 outline-none transition-all placeholder:text-black/25 focus:border-jade/45 focus:ring-[3px] focus:ring-jade/12"
              />
              <Button
                type="button"
                size="sm"
                onClick={handleAutoExpand}
                disabled={!trimmedName}
                className="h-9 shrink-0 rounded-lg px-3 text-[12px] font-semibold"
                title={
                  trimmedName
                    ? "根据特征关键词自动生成 summary + 行为规则"
                    : "请先填名字"
                }
              >
                <Wand2 className="mr-1.5 h-3.5 w-3.5" />
                智能扩写
              </Button>
            </div>
            <span className="text-[10.5px] leading-4 text-black/35">
              扩写会从内置的特征模板里召回匹配的语气 / 协作 / 输出规则；如果没有匹配关键词，会用通用骨架兜底。
            </span>
          </div>

          {/* Summary (filled by auto-expand or manually) */}
          <label className="grid gap-1.5">
            <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
              Summary · 一句话简介
            </span>
            <input
              type="text"
              value={summary}
              onChange={(event) => setSummary(event.target.value)}
              placeholder="点击上方「智能扩写」自动填入；也可手动填写"
              maxLength={200}
              className="h-9 rounded-lg border border-black/[0.08] bg-white px-3 text-[13px] tracking-tight text-foreground/90 outline-none transition-all placeholder:text-black/25 focus:border-jade/45 focus:ring-[3px] focus:ring-jade/12"
            />
          </label>

          {/* Avatar picker */}
          <div className="grid gap-2">
            <div className="flex items-center justify-between">
              <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
                Avatar · 选个头像
              </span>
              {avatarId ? (
                <button
                  type="button"
                  onClick={() => setAvatarId(null)}
                  className="text-[10.5px] font-medium text-black/45 hover:text-foreground"
                >
                  清除选择
                </button>
              ) : null}
            </div>
            <div className="grid grid-cols-3 gap-2 sm:grid-cols-6">
              {PERSONA_AVATAR_LIBRARY.map((entry) => {
                const active = avatarId === entry.id;
                const inUse = avatarsInUse.has(entry.id);
                return (
                  <button
                    key={entry.id}
                    type="button"
                    onClick={() => setAvatarId(entry.id)}
                    title={`${entry.label} · ${entry.vibe}${inUse ? " · 已被其他 Persona 使用" : ""}`}
                    className={cn(
                      "group relative aspect-square overflow-hidden rounded-xl border bg-[#f5efe5] transition-all",
                      active
                        ? "border-jade/55 shadow-[0_0_0_4px_rgba(16,185,129,0.12),0_4px_12px_rgba(15,23,42,0.07)]"
                        : "border-black/[0.07] hover:-translate-y-0.5 hover:border-black/[0.15] hover:shadow-[0_4px_12px_rgba(15,23,42,0.07)]",
                      inUse && !active && "opacity-55",
                    )}
                  >
                    <img
                      src={PERSONA_AVATAR_BY_ID[entry.id]}
                      alt={entry.label}
                      className="h-full w-full object-cover object-[50%_28%] select-none"
                      draggable={false}
                    />
                    {active ? (
                      <span className="absolute right-1 top-1 inline-flex h-5 w-5 items-center justify-center rounded-full bg-jade text-white shadow-[0_2px_6px_rgba(16,185,129,0.4)] ring-2 ring-white">
                        <Check className="h-3 w-3" strokeWidth={3} />
                      </span>
                    ) : null}
                  </button>
                );
              })}
            </div>
            <span className="text-[10.5px] text-black/35">
              不选头像也可以保存 — 列表里会显示一个占位插画。
            </span>
          </div>

          {/* Behavior rules — collapsed by default; auto-opens after expand */}
          <details
            open={advancedOpen}
            onToggle={(event) =>
              setAdvancedOpen((event.target as HTMLDetailsElement).open)
            }
            className="rounded-xl border border-black/[0.07] bg-black/[0.015] px-3.5 py-3 [&_summary::-webkit-details-marker]:hidden"
          >
            <summary className="flex cursor-pointer items-center justify-between text-[11.5px] font-medium text-foreground/75">
              <span className="inline-flex items-center gap-1.5">
                <Sparkles className="h-3 w-3 text-jade" />
                行为规则（点击展开 / 折叠）
              </span>
              <span className="text-[10.5px] text-black/35">
                {advancedOpen ? "折叠" : "展开"}
              </span>
            </summary>
            <div className="mt-3 grid gap-3">
              <FieldGroup label="Tone Rules · 语气规则">
                <Textarea
                  value={toneText}
                  onChange={(event) => setToneText(event.target.value)}
                  className={editorClass}
                  placeholder={"- 温和但有边界\n- 少用破折号"}
                />
              </FieldGroup>
              <FieldGroup label="Collaboration Rules · 协作方式">
                <Textarea
                  value={collabText}
                  onChange={(event) => setCollabText(event.target.value)}
                  className={editorClass}
                  placeholder={"- 先问需求，再给方案\n- 阻塞立即同步"}
                />
              </FieldGroup>
              <FieldGroup label="Output Preferences · 输出偏好">
                <Textarea
                  value={outputText}
                  onChange={(event) => setOutputText(event.target.value)}
                  className={editorClass}
                  placeholder={"- 结论先行\n- 复杂内容用结构化清单"}
                />
              </FieldGroup>
            </div>
          </details>
        </div>

        {/* Footer — pinned at the bottom, always visible */}
        <DialogFooter className="shrink-0 border-t border-black/[0.06] bg-white/95 px-6 py-3 backdrop-blur-sm">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={saving}
            className="h-9 rounded-lg border-black/[0.09] bg-black/[0.025] text-[12.5px] hover:bg-black/[0.05]"
          >
            取消
          </Button>
          <Button
            type="button"
            onClick={() => void handleSave()}
            disabled={!canSave}
            className="h-9 rounded-lg text-[12.5px]"
          >
            <Bot className="mr-1.5 h-3.5 w-3.5" />
            {saving
              ? "保存中..."
              : isEditMode
                ? "保存修改"
                : "创建 Persona"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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
    <div className="rounded-lg border border-black/[0.06] bg-white px-3 py-2.5">
      <div className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
        {label}
      </div>
      <div className="mt-2">{children}</div>
    </div>
  );
}

const editorClass =
  "min-h-[88px] rounded-md border-black/[0.08] bg-black/[0.015] text-[12.5px] leading-5 focus-visible:ring-[3px] focus-visible:ring-jade/15";
