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
import { ThumbsDown, Sparkles, AlertCircle } from "lucide-react";
import { toast } from "sonner";
import {
  learnedTraitsDisagree,
  learnedTraitsList,
  type LearnedTraitDto,
} from "@/lib/tauri";
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
      <div className="rounded-lg border border-dashed border-black/10 bg-black/[0.02] px-4 py-6 text-center text-[11.5px] text-muted-foreground">
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
            className="group flex items-start gap-3 rounded-xl border border-black/[0.06] bg-white/60 px-3.5 py-3 transition-colors hover:bg-white/90"
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
                <div className="h-1 flex-1 overflow-hidden rounded-full bg-black/[0.05]">
                  <div
                    className={cn(
                      "h-full rounded-full transition-all duration-300",
                      pct >= 70
                        ? "bg-emerald-500/70"
                        : pct >= 40
                          ? "bg-amber-500/70"
                          : "bg-black/30",
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
                "flex h-7 shrink-0 items-center gap-1 rounded-lg border border-black/[0.08] px-2.5 text-[11px] font-medium",
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
    </div>
  );
}
