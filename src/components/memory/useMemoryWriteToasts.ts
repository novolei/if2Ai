/**
 * useMemoryWriteToasts — show a subtle "AI 记下了新东西" toast after
 * each turn that produced accepted memory writes.
 *
 * Memory System Audit P2 #10: closes the audit's "用户感知不到 memory
 * 在工作" finding without redesigning the chat header. The runtime
 * projection already populates `memory.lastAfterTurn` on every turn
 * end (M3-C); this hook turns that signal into a single, non-intrusive
 * sonner toast so the user sees value flowing.
 *
 * Behavior:
 *   - Fires once per `lastAfterTurn.decidedAt` (uses the timestamp as
 *     the dedup key so React StrictMode double-effects don't double-toast).
 *   - Skips silent turns: only shows when `acceptedCount > 0`.
 *   - Auto-dismisses after 4 seconds (sonner default).
 *   - Clicking the toast does nothing destructive — there's no navigation
 *     guarantee from the chat surface, so we keep it informational.
 */

import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { useRuntimeProjectionSelector } from "@/runtime-projection";

export function useMemoryWriteToasts() {
  const lastAfterTurn = useRuntimeProjectionSelector((s) => s.memory.lastAfterTurn);
  const lastShownKeyRef = useRef<string | null>(null);

  useEffect(() => {
    if (!lastAfterTurn) return;

    // Dedup: a single decidedAt timestamp = one toast per logical turn end.
    const key = lastAfterTurn.decidedAt;
    if (lastShownKeyRef.current === key) return;
    lastShownKeyRef.current = key;

    // Silent turns (no accepted writes) shouldn't pop a toast.
    const accepted = lastAfterTurn.acceptedCount;
    if (accepted <= 0) return;

    const rejectedHint =
      lastAfterTurn.rejectedCount > 0
        ? ` · ${lastAfterTurn.rejectedCount} 条被策略拒绝`
        : "";
    const conflictsHint =
      lastAfterTurn.conflictsCount > 0
        ? ` · 解决冲突 ${lastAfterTurn.conflictsCount}`
        : "";

    toast.success(`AI 记住了 ${accepted} 件事`, {
      description: `本轮写入了 ${accepted} 条记忆${rejectedHint}${conflictsHint}`,
      duration: 4000,
    });
  }, [lastAfterTurn]);
}
