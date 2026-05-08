import { useEffect } from "react";
import { toast } from "sonner";

/**
 * Subscribes to `chat_compact_completed` so the user gets a single
 * sonner line whenever the backend's `stream_finalize` crosses
 * `IF2AI_AUTO_COMPACT_THRESHOLD` (default 85%) and spawns a background
 * fold. Extracted from `App.tsx` (GF-03 PR-1) so the runtime/effect
 * layer stays composable.
 *
 * Reports with `didCompact === false` are ignored (the backend still
 * emits a heartbeat record after a no-op decision).
 */
export function useAutoCompactToast(): void {
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void (async () => {
      const { listenChatCompactCompleted } = await import("@/lib/tauri");
      unlisten = await listenChatCompactCompleted((report) => {
        if (!report.didCompact) return;
        const freed =
          report.freedTokens > 0
            ? `${(report.freedTokens / 1000).toFixed(1)}k`
            : "0";
        toast.success(
          `上下文接近上限，已自动压缩 ${report.summarizedMessages} 条消息`,
          {
            description: `释放约 ${freed} tokens · 摘要：${report.summaryExcerpt}…`,
            duration: 5000,
          },
        );
      });
    })();
    return () => {
      unlisten?.();
    };
  }, []);
}
