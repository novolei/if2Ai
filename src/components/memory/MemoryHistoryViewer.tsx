/**
 * MEM-MOD-P6 — Memory entry history viewer.
 *
 * Shows the temporal sequence of values a memory key has held — every
 * UPDATE / CONSOLIDATE archives the previous value into
 * `memory_entry_history` (migration v3) so the agent (and the user)
 * can answer "what did I know at time T".
 *
 * Embedded as a small inline panel rather than a full modal so it can
 * sit underneath the current value in MemoryCard / Memory Browser
 * without context-switching.
 */

import { useCallback, useEffect, useState } from "react";
import { History, AlertCircle } from "lucide-react";
import {
  memoryHistory,
  type MemoryHistoryEntryDto,
} from "@/lib/tauri";

interface MemoryHistoryViewerProps {
  memoryKey: string;
  /** Optional: fold the panel into a `<details>` so the host page
   *  doesn't have to manage open/closed state. */
  collapsible?: boolean;
}

function formatTime(rfc3339: string): string {
  try {
    return new Date(rfc3339).toLocaleString("zh-CN", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return rfc3339;
  }
}

function sourceBadge(source: string): { label: string; className: string } {
  switch (source) {
    case "update":
      return { label: "Update", className: "bg-blue-100 text-blue-700" };
    case "consolidate":
      return {
        label: "Consolidate",
        className: "bg-violet-100 text-violet-700",
      };
    default:
      return { label: source, className: "bg-muted text-foreground/70" };
  }
}

export function MemoryHistoryViewer({
  memoryKey,
  collapsible = true,
}: MemoryHistoryViewerProps) {
  const [history, setHistory] = useState<MemoryHistoryEntryDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await memoryHistory(memoryKey);
      setHistory(list);
      setLoaded(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [memoryKey]);

  useEffect(() => {
    if (!collapsible) {
      void load();
    }
  }, [collapsible, load]);

  const body = (() => {
    if (loading) {
      return (
        <div className="text-[11px] text-muted-foreground">加载历史…</div>
      );
    }
    if (error) {
      return (
        <div className="flex items-start gap-2 rounded-lg border border-amber-300/40 bg-amber-50/60 px-2.5 py-1.5 text-[11px] text-amber-900">
          <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          <span>{error}</span>
        </div>
      );
    }
    if (loaded && history.length === 0) {
      return (
        <div className="text-[11px] text-muted-foreground/80">
          这条记忆还没有被修改过 — 历史为空。
        </div>
      );
    }
    return (
      <ol className="relative ml-1 flex flex-col gap-2 border-l border-border pl-3">
        {history.map((h, idx) => {
          const badge = sourceBadge(h.source);
          return (
            <li key={`${h.valid_from}-${idx}`} className="relative">
              <span className="absolute -left-[15px] top-1.5 h-2 w-2 rounded-full bg-muted-foreground/35" />
              <div className="flex items-center gap-2 text-[10.5px] text-muted-foreground">
                <span
                  className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${badge.className}`}
                >
                  {badge.label}
                </span>
                <span title={h.valid_to}>{formatTime(h.valid_to)} 被覆盖</span>
                <span className="opacity-60">·</span>
                <span title={`valid_from: ${h.valid_from}`}>
                  曾自 {formatTime(h.valid_from)} 起生效
                </span>
              </div>
              <div className="mt-1 whitespace-pre-wrap break-words rounded-lg border border-border bg-muted/30 px-2.5 py-1.5 text-[11.5px] leading-snug text-foreground/85">
                {h.content}
              </div>
            </li>
          );
        })}
      </ol>
    );
  })();

  if (!collapsible) {
    return <div className="flex flex-col gap-1.5">{body}</div>;
  }

  return (
    <details
      className="group rounded-lg border border-border bg-muted/25"
      onToggle={(e) => {
        const open = (e.currentTarget as HTMLDetailsElement).open;
        if (open && !loaded && !loading) void load();
      }}
    >
      <summary className="flex cursor-pointer list-none items-center gap-1.5 px-3 py-2 text-[11.5px] font-medium text-foreground/80 transition-colors hover:bg-accent hover:text-accent-foreground">
        <History className="h-3.5 w-3.5 text-violet-600/80" />
        查看历史版本
        <span className="ml-auto text-[10.5px] text-muted-foreground group-open:hidden">
          展开
        </span>
        <span className="ml-auto hidden text-[10.5px] text-muted-foreground group-open:inline">
          收起
        </span>
      </summary>
      <div className="border-t border-border px-3 py-2.5">{body}</div>
    </details>
  );
}
