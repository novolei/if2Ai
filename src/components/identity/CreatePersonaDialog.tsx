import { useEffect, useMemo, useState } from "react";
import { Bot, Check } from "lucide-react";
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
}

/** Convert "Quiet Mind / 静心" → "quiet-mind". Keeps lowercase ASCII + dashes. */
function slugify(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}\s-]/gu, "")
    .replace(/\s+/g, "-")
    .replace(/-{2,}/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
}

/** Heuristic: split lines, drop empty / leading bullets. */
function linesFromText(value: string): string[] {
  return value
    .split("\n")
    .map((line) => line.replace(/^\s*[-*]\s*/, "").trim())
    .filter(Boolean);
}

export function CreatePersonaDialog({
  open,
  onOpenChange,
  catalog,
  defaultSoulId,
  currentPack,
  onSaved,
}: CreatePersonaDialogProps) {
  const [name, setName] = useState("");
  const [id, setId] = useState("");
  const [idTouched, setIdTouched] = useState(false);
  const [soulId, setSoulId] = useState<string>("");
  const [summary, setSummary] = useState("");
  const [avatarId, setAvatarId] = useState<string | null>(null);
  const [toneText, setToneText] = useState("");
  const [collabText, setCollabText] = useState("");
  const [outputText, setOutputText] = useState("");
  const [saving, setSaving] = useState(false);

  // Reset form on open and seed soul / avatar with sensible defaults.
  useEffect(() => {
    if (!open) return;
    setName("");
    setId("");
    setIdTouched(false);
    setSoulId(
      defaultSoulId && defaultSoulId !== "auto"
        ? defaultSoulId
        : (catalog?.souls[0]?.id ?? ""),
    );
    setSummary("");
    setAvatarId(null);
    setToneText("");
    setCollabText("");
    setOutputText("");
  }, [open, defaultSoulId, catalog]);

  // Auto-derive id from name unless the user has touched the id field.
  useEffect(() => {
    if (idTouched) return;
    setId(slugify(name));
  }, [name, idTouched]);

  // Existing persona ids (built-in + custom in pack) — prevent collisions.
  const takenIds = useMemo(() => {
    const set = new Set<string>();
    catalog?.personas.forEach((persona) => set.add(persona.id));
    Object.keys(currentPack.custom_personas ?? {}).forEach((key) =>
      set.add(key),
    );
    return set;
  }, [catalog, currentPack]);

  // Avatars already used by other custom personas in the pack — show
  // them as still selectable but visually faded so the user knows.
  const avatarsInUse = useMemo(() => {
    const set = new Set<string>();
    Object.values(currentPack.custom_personas ?? {}).forEach((persona) => {
      if (persona.avatar_id) set.add(persona.avatar_id);
    });
    return set;
  }, [currentPack]);

  const trimmedName = name.trim();
  const trimmedId = id.trim();
  const trimmedSummary = summary.trim();

  const idError = (() => {
    if (!trimmedId) return null;
    if (!/^[a-z0-9][a-z0-9-]*$/.test(trimmedId)) {
      return "id 只能含小写字母 / 数字 / 短横线";
    }
    if (takenIds.has(trimmedId)) {
      return "id 已被其他 persona 占用";
    }
    return null;
  })();

  const canSave =
    trimmedName.length > 0 &&
    trimmedId.length > 0 &&
    !idError &&
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
          [trimmedId]: newPersona,
        },
      };
      const saved = await setIdentityPack(nextPack);
      toast.success("自定义 Persona 已创建", {
        description: `${trimmedName} 现在可在 Persona 列表里点击切换。`,
      });
      await onSaved(saved);
      onOpenChange(false);
    } catch (error) {
      toast.error("创建 Persona 失败", { description: String(error) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="text-[16px] font-semibold tracking-tight">
            新建 Persona
          </DialogTitle>
          <DialogDescription className="text-[12px] leading-[1.6]">
            Persona 是 Soul 之上的<span className="font-medium text-foreground/70"> 表达模式</span>。
            填好后会以 custom persona 形式保存到 identity-pack.json，并立即出现在 Persona 列表中。
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-4">
          {/* Name + Id */}
          <div className="grid gap-3 sm:grid-cols-2">
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
            </label>
            <label className="grid gap-1.5">
              <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
                Id · 唯一标识
              </span>
              <input
                type="text"
                value={id}
                onChange={(event) => {
                  setId(event.target.value);
                  setIdTouched(true);
                }}
                placeholder="quiet-mind"
                maxLength={48}
                className={cn(
                  "h-9 rounded-lg border bg-white px-3 font-mono text-[12.5px] text-foreground/90 outline-none transition-all placeholder:text-black/25",
                  idError
                    ? "border-red-300 focus:border-red-400 focus:ring-[3px] focus:ring-red-100"
                    : "border-black/[0.08] focus:border-jade/45 focus:ring-[3px] focus:ring-jade/12",
                )}
              />
              {idError ? (
                <span className="text-[10.5px] text-red-600">{idError}</span>
              ) : (
                <span className="text-[10.5px] text-black/35">
                  自动从名字推导，可手动修改；保存后不可再改
                </span>
              )}
            </label>
          </div>

          {/* Soul + Summary */}
          <div className="grid gap-3 sm:grid-cols-[160px_minmax(0,1fr)]">
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
            <label className="grid gap-1.5">
              <span className="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-black/40">
                Summary · 一句话简介
              </span>
              <input
                type="text"
                value={summary}
                onChange={(event) => setSummary(event.target.value)}
                placeholder="例：温柔陪伴 + 深度洞察"
                maxLength={160}
                className="h-9 rounded-lg border border-black/[0.08] bg-white px-3 text-[13px] tracking-tight text-foreground/90 outline-none transition-all placeholder:text-black/25 focus:border-jade/45 focus:ring-[3px] focus:ring-jade/12"
              />
            </label>
          </div>

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

          {/* Optional rules */}
          <details className="rounded-xl border border-black/[0.07] bg-black/[0.015] px-3.5 py-3 [&_summary::-webkit-details-marker]:hidden">
            <summary className="flex cursor-pointer items-center justify-between text-[11.5px] font-medium text-foreground/75">
              <span>可选 · 行为规则（保存后随时可在「可编辑 Identity Pack」继续完善）</span>
              <span className="text-[10.5px] text-black/35 group-open:hidden">
                展开
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

        <DialogFooter>
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
            {saving ? "保存中..." : "创建 Persona"}
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
