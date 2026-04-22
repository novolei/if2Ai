import { useEffect, useMemo, useState } from "react";
import { Shield, Sparkles } from "lucide-react";
import { toast } from "sonner";
import { getIdentityCatalog, type SessionIdentityInput } from "@/api/identity";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { PromptControlCatalog } from "@/lib/tauri";
import { cn } from "@/lib/utils";

export interface SessionIdentitySnapshot {
  id: string;
  title?: string;
  soul_id?: string | null;
  persona_id?: string | null;
}

interface SessionIdentityEditorProps {
  session: SessionIdentitySnapshot | null;
  onUpdate: (
    sessionId: string,
    identity: SessionIdentityInput,
  ) => Promise<void>;
  catalog?: PromptControlCatalog | null;
  variant?: "header" | "panel";
}

function resolveIdentityLabel(
  catalog: PromptControlCatalog | null,
  session: SessionIdentitySnapshot | null,
): string {
  if (!catalog || !session) return "Auto Identity";
  const soul = session.soul_id
    ? catalog.souls.find((item) => item.id === session.soul_id)
    : null;
  const persona = session.persona_id
    ? catalog.personas.find((item) => item.id === session.persona_id)
    : null;
  if (soul && persona) return `${soul.name} / ${persona.name}`;
  if (persona) return persona.name;
  if (soul) return soul.name;
  return "Auto Identity";
}

export function SessionIdentityEditor({
  session,
  onUpdate,
  catalog: catalogProp,
  variant = "panel",
}: SessionIdentityEditorProps) {
  const [catalog, setCatalog] = useState<PromptControlCatalog | null>(
    catalogProp ?? null,
  );
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [selectedSoulId, setSelectedSoulId] = useState<string>("auto");
  const [selectedPersonaId, setSelectedPersonaId] = useState<string>("auto");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (catalogProp) {
      setCatalog(catalogProp);
      return;
    }
    let cancelled = false;
    void getIdentityCatalog()
      .then((nextCatalog) => {
        if (!cancelled) setCatalog(nextCatalog);
      })
      .catch(() => {
        if (!cancelled) setCatalog(null);
      });
    return () => {
      cancelled = true;
    };
  }, [catalogProp]);

  useEffect(() => {
    setSelectedSoulId(session?.soul_id ?? "auto");
    setSelectedPersonaId(session?.persona_id ?? "auto");
  }, [session?.id, session?.persona_id, session?.soul_id]);

  const quickPersonas = useMemo(() => {
    if (!catalog) return [];
    if (session?.soul_id) {
      return catalog.personas.filter(
        (persona) => persona.soul_id === session.soul_id,
      );
    }
    return catalog.personas;
  }, [catalog, session?.soul_id]);

  const advancedPersonas = useMemo(() => {
    if (!catalog) return [];
    if (selectedSoulId === "auto") return [];
    return catalog.personas.filter(
      (persona) => persona.soul_id === selectedSoulId,
    );
  }, [catalog, selectedSoulId]);

  const currentLabel = useMemo(
    () => resolveIdentityLabel(catalog, session),
    [catalog, session],
  );

  const saveIdentity = async (identity: SessionIdentityInput) => {
    if (!session) return;
    setSaving(true);
    try {
      await onUpdate(session.id, identity);
      toast.success("当前会话 Identity 已更新");
      setAdvancedOpen(false);
    } catch (error) {
      toast.error("更新当前会话 Identity 失败", {
        description: String(error),
      });
    } finally {
      setSaving(false);
    }
  };

  const handleQuickPersonaChange = async (value: string) => {
    if (!catalog || !session) return;
    if (value === "auto") {
      await saveIdentity({
        soul_id: session.soul_id ?? null,
        persona_id: null,
      });
      return;
    }
    const persona = catalog.personas.find((item) => item.id === value);
    if (!persona) return;
    await saveIdentity({
      soul_id: session.soul_id ?? persona.soul_id,
      persona_id: persona.id,
    });
  };

  const tone =
    variant === "header"
      ? "border-black/[0.07] bg-white/76"
      : "border-black/[0.06] bg-white/82";

  if (!session) {
    return variant === "header" ? null : (
      <div className="rounded-[20px] border border-dashed border-black/[0.08] bg-black/[0.02] px-4 py-5 text-[12px] text-muted-foreground">
        当前没有活跃 session，无法设置 session-level persona / soul。
      </div>
    );
  }

  return (
    <>
      <div
        className={cn(
          "rounded-[18px] border px-3.5 py-3",
          tone,
          variant === "header" ? "window-no-drag" : "",
        )}
        data-window-no-drag={variant === "header" ? "true" : undefined}
      >
        <div
          className={cn(
            "flex gap-3",
            variant === "header"
              ? "items-center"
              : "items-start justify-between",
          )}
        >
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="inline-flex h-7 w-7 items-center justify-center rounded-full bg-amber-50 text-amber-700">
                <Sparkles className="h-3.5 w-3.5" />
              </span>
              <div className="min-w-0">
                <div className="truncate text-[12.5px] font-semibold tracking-tight text-foreground">
                  {variant === "header"
                    ? currentLabel
                    : "Current Session Identity"}
                </div>
                <div className="truncate text-[11px] text-muted-foreground">
                  {variant === "header"
                    ? (session.title ?? "当前会话")
                    : session.title
                      ? `Session: ${session.title}`
                      : "当前会话后续回复将使用这个 identity override"}
                </div>
              </div>
            </div>
          </div>
          {variant !== "header" ? (
            <div className="rounded-full border border-black/[0.06] bg-black/[0.03] px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
              Session Override
            </div>
          ) : null}
        </div>

        <div
          className={cn(
            "mt-3 flex gap-2",
            variant === "header" ? "items-center" : "items-end",
          )}
        >
          <div className="min-w-[170px] flex-1">
            <div className="mb-1 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground/72">
              Persona
            </div>
            <Select
              value={session.persona_id ?? "auto"}
              onValueChange={(value) => {
                void handleQuickPersonaChange(value);
              }}
              disabled={saving || !catalog}
            >
              <SelectTrigger className="h-9 rounded-2xl border-black/[0.08] bg-black/[0.02] text-[12.5px]">
                <SelectValue placeholder="选择 Persona" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="auto">Auto / None</SelectItem>
                {quickPersonas.map((persona) => {
                  const soulName =
                    catalog?.souls.find((item) => item.id === persona.soul_id)
                      ?.name ?? persona.soul_id;
                  return (
                    <SelectItem key={persona.id} value={persona.id}>
                      {session.soul_id
                        ? persona.name
                        : `${persona.name} · ${soulName}`}
                    </SelectItem>
                  );
                })}
              </SelectContent>
            </Select>
          </div>

          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-9 rounded-2xl border-black/[0.08] bg-black/[0.02] px-3"
            onClick={() => setAdvancedOpen(true)}
            disabled={!catalog}
          >
            <Shield className="mr-1.5 h-3.5 w-3.5" />
            Soul
          </Button>
        </div>
      </div>

      <Dialog open={advancedOpen} onOpenChange={setAdvancedOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Advanced Session Identity</DialogTitle>
            <DialogDescription>
              Persona 是常规切换入口。Soul 会改变当前 session 后续回复的
              identity 基线，因此放在高级操作里。
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="rounded-2xl border border-amber-200/70 bg-amber-50 px-3.5 py-3 text-[12px] leading-5 text-amber-800">
              这次修改只影响当前会话后续回复，不会改写全局默认 Soul / Persona。
            </div>

            <div className="grid gap-3">
              <label className="space-y-2">
                <div className="text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground/72">
                  Soul
                </div>
                <Select
                  value={selectedSoulId}
                  onValueChange={(value) => {
                    setSelectedSoulId(value);
                    if (value === "auto") {
                      setSelectedPersonaId("auto");
                    } else if (
                      !catalog?.personas.some(
                        (persona) =>
                          persona.id === selectedPersonaId &&
                          persona.soul_id === value,
                      )
                    ) {
                      setSelectedPersonaId("auto");
                    }
                  }}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="auto">Auto / Follow Defaults</SelectItem>
                    {(catalog?.souls ?? []).map((soul) => (
                      <SelectItem key={soul.id} value={soul.id}>
                        {soul.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </label>

              <label className="space-y-2">
                <div className="text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground/72">
                  Persona
                </div>
                <Select
                  value={selectedPersonaId}
                  onValueChange={setSelectedPersonaId}
                  disabled={selectedSoulId === "auto"}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="选择 Persona" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="auto">Auto / None</SelectItem>
                    {advancedPersonas.map((persona) => (
                      <SelectItem key={persona.id} value={persona.id}>
                        {persona.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </label>
            </div>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setAdvancedOpen(false)}
            >
              取消
            </Button>
            <Button
              type="button"
              disabled={saving}
              onClick={() => {
                void saveIdentity({
                  soul_id: selectedSoulId === "auto" ? null : selectedSoulId,
                  persona_id:
                    selectedPersonaId === "auto" ? null : selectedPersonaId,
                });
              }}
            >
              {saving ? "保存中..." : "保存当前会话 Identity"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

export default SessionIdentityEditor;
