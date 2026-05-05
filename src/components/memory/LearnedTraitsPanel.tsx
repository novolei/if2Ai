/**
 * MEM-MOD-P7 — Cross-session learned traits surface.
 *
 * Lists the durable observations the agent has accumulated about the
 * user across sessions (extracted by the reflection loop's distiller),
 * each with a confidence bar, evidence count, and a "我不同意" button
 * that calls {@link learnedTraitsDisagree} and removes the trait from
 * future prompts (the row is preserved for audit).
 *
 * Designed to live inside `MemorySettingsPage` as one `SettingsSurface`
 * — does NOT manage its own card chrome. Auto-refreshes when the user
 * disagrees with a trait so the list shrinks immediately.
 */

import { useCallback, useEffect, useState } from "react";
import { ThumbsDown, Sparkles, AlertCircle, Award, ChevronRight } from "lucide-react";
import { toast } from "sonner";
import {
  learnedTraitsDisagree,
  learnedTraitsList,
  type LearnedTraitDto,
} from "@/lib/tauri";
import { useProceduralMemories } from "@/api/memory";
import { cn } from "@/lib/utils";

function formatRelative(rfc3339: string): string {
  try {
    const ts = new Date(rfc3339).getTime();
    const diffSec = Math.max(0, (Date.now() - ts) / 1000);
    if (diffSec < 60) return "刚刚";
    if (diffSec < 3600) return `${Math.floor(diffSec / 60)} 分钟前`;
    if (diffSec < 86400) return `${Math.floor(diffSec / 3600)} 小时前`;
    return `${Math.floor(diffSec / 86400)} 天前`;
  } catch {
    return rfc3339;
  }
}

export function LearnedTraitsPanel() {
  const [traits, setTraits] = useState<LearnedTraitDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyIds, setBusyIds] = useState<Set<number>>(new Set());

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await learnedTraitsList();
      setTraits(list);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleDisagree = useCallback(
    async (id: number, label: string) => {
      setBusyIds((prev) => new Set(prev).add(id));
      try {
        await learnedTraitsDisagree(id);
        toast.success("已撤回这条特征", {
          description: `Agent 之后的 prompt 中不会再出现："${label}"`,
        });
        // Optimistic + reconcile
        setTraits((prev) => prev.filter((t) => t.id !== id));
      } catch (err) {
        toast.error("撤回失败", { description: String(err) });
      } finally {
        setBusyIds((prev) => {
          const next = new Set(prev);
          next.delete(id);
          return next;
        });
      }
    },
    [],
  );

  if (loading) {
    return (
      <div className="text-[11.5px] text-muted-foreground">加载学到的特征…</div>
    );
  }

  if (error) {
    return (
      <div className="flex items-start gap-2 rounded-lg border border-amber-300/40 bg-amber-50/50 px-3 py-2 text-[11.5px] text-amber-900">
        <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
        <div>
          <div className="font-medium">无法读取 learned_traits</div>
          <div className="mt-0.5 text-amber-900/70">{error}</div>
        </div>
      </div>
    );
  }

  if (traits.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-border bg-muted/30 px-4 py-6 text-center text-[11.5px] text-muted-foreground">
        <Sparkles className="mx-auto mb-2 h-4 w-4 text-violet-500/70" />
        <div>暂时没有累积的跨 session 特征。</div>
        <div className="mt-1 text-[10.5px] opacity-70">
          Agent 在每个 session 结束时会基于 reflection 提炼 1–3 条「关于你」的观察。
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {traits.map((t) => {
        const pct = Math.round(t.confidence * 100);
        const busy = busyIds.has(t.id);
        return (
          <div
            key={t.id}
            className="group flex items-start gap-3 rounded-xl border border-border bg-card/70 px-3.5 py-3 transition-colors hover:bg-accent/45"
          >
            <div className="min-w-0 flex-1">
              <div className="text-[12.5px] font-medium leading-snug text-foreground/90">
                {t.trait_text}
              </div>
              <div className="mt-1.5 flex items-center gap-3 text-[10.5px] text-muted-foreground">
                <span>证据 ×{t.evidence_count}</span>
                <span>·</span>
                <span>更新 {formatRelative(t.last_updated_at)}</span>
                {t.source_session ? (
                  <>
                    <span>·</span>
                    <span
                      className="font-mono text-[10px] opacity-70"
                      title={t.source_session}
                    >
                      {t.source_session.slice(0, 8)}
                    </span>
                  </>
                ) : null}
              </div>
              <div className="mt-1.5 flex items-center gap-2">
                <div className="h-1 flex-1 overflow-hidden rounded-full bg-muted">
                  <div
                    className={cn(
                      "h-full rounded-full transition-all duration-300",
                      pct >= 70
                        ? "bg-emerald-500/70"
                        : pct >= 40
                          ? "bg-amber-500/70"
                          : "bg-muted-foreground/45",
                    )}
                    style={{ width: `${pct}%` }}
                  />
                </div>
                <span className="font-mono text-[10.5px] tabular-nums text-muted-foreground">
                  {pct}%
                </span>
              </div>
            </div>
            <button
              type="button"
              disabled={busy}
              onClick={() => void handleDisagree(t.id, t.trait_text)}
              className={cn(
                "flex h-7 shrink-0 items-center gap-1 rounded-lg border border-border px-2.5 text-[11px] font-medium",
                "text-muted-foreground transition-colors",
                "hover:border-red-300/60 hover:bg-red-50/60 hover:text-red-700",
                "disabled:cursor-not-allowed disabled:opacity-40",
              )}
              title="标记为我不同意 — 之后 prompt 不会再出现这条"
            >
              <ThumbsDown className="h-3 w-3" />
              {busy ? "撤回中…" : "我不同意"}
            </button>
          </div>
        );
      })}

      {/* ── 从经验中学到的规则 (Procedural) ── */}
      <ProceduralRulesSection />
    </div>
  );
}

// ── Procedural rules mini-section ───────────────────────────────────

function ProceduralRulesSection() {
  const { procedures, loading } = useProceduralMemories();

  if (loading || procedures.length === 0) return null;

  // Show top 5 by trust_score
  const topRules = [...procedures]
    .sort((a, b) => b.trust_score - a.trust_score)
    .slice(0, 5);

  return (
    <div className="mt-3 border-t border-border/50 pt-3">
      <div className="mb-2 flex items-center gap-2">
        <Award className="h-3.5 w-3.5 text-emerald-600" />
        <span className="text-[11px] font-semibold text-foreground/80">
          从经验中学到的规则
        </span>
        <span className="rounded-full bg-emerald-500/10 px-1.5 py-0.5 text-[10px] text-emerald-700">
          {procedures.length}
        </span>
      </div>
      <div className="flex flex-col gap-1.5">
        {topRules.map((rule) => {
          const pct = Math.round(rule.trust_score * 100);
          return (
            <div
              key={rule.key}
              className="rounded-lg border border-emerald-200/50 bg-emerald-50/30 px-3 py-2"
            >
              <div className="text-[11.5px] leading-snug text-foreground/80">
                {rule.content.length > 100
                  ? rule.content.slice(0, 100) + "…"
                  : rule.content}
              </div>
              <div className="mt-1 flex items-center gap-2 text-[10px] text-muted-foreground">
                <span
                  className={cn(
                    "font-mono tabular-nums",
                    pct > 50
                      ? "text-emerald-600"
                      : pct > 20
                        ? "text-amber-600"
                        : "text-red-600",
                  )}
                >
                  {pct}%
                </span>
                <span>·</span>
                <span>更新 {formatRelative(rule.updated_at)}</span>
              </div>
            </div>
          );
        })}
      </div>
      {procedures.length > 5 && (
        <div className="mt-2 text-center">
          <span className="inline-flex items-center gap-0.5 text-[10.5px] text-primary">
            在「进化」标签页查看全部 {procedures.length} 条
            <ChevronRight className="h-3 w-3" />
          </span>
        </div>
      )}
    </div>
  );
}
