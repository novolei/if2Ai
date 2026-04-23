import { useEffect, useState } from "react";
import { Zap } from "lucide-react";

import { cn } from "@/lib/utils";

/** Reasoning effort tiers accepted by OpenAI o1 / o3 / GPT-5. */
export type ThinkingLevel = "off" | "low" | "medium" | "high";

const STORAGE_KEY = "IF2AI_THINKING_LEVEL";
const DEFAULT_LEVEL: ThinkingLevel = "medium";

function readStored(): ThinkingLevel {
  if (typeof window === "undefined") return DEFAULT_LEVEL;
  const raw = window.localStorage.getItem(STORAGE_KEY);
  return raw === "off" || raw === "low" || raw === "medium" || raw === "high"
    ? raw
    : DEFAULT_LEVEL;
}

/** Hook + cross-tab sync for the user-selected reasoning effort tier. */
export function useThinkingLevel(): [ThinkingLevel, (next: ThinkingLevel) => void] {
  const [level, setLevel] = useState<ThinkingLevel>(readStored);

  useEffect(() => {
    const handler = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) setLevel(readStored());
    };
    window.addEventListener("storage", handler);
    return () => window.removeEventListener("storage", handler);
  }, []);

  const update = (next: ThinkingLevel) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, next);
      window.dispatchEvent(
        new StorageEvent("storage", { key: STORAGE_KEY, newValue: next }),
      );
    }
    setLevel(next);
  };

  return [level, update];
}

interface ThinkingLevelButtonProps {
  /** Hide entirely when the active model does not accept `reasoning_effort`. */
  visible: boolean;
  className?: string;
}

const ORDER: ThinkingLevel[] = ["off", "low", "medium", "high"];
const LABEL: Record<ThinkingLevel, string> = {
  off: "关闭推理",
  low: "低强度推理",
  medium: "中强度推理",
  high: "高强度推理",
};
const SHORT: Record<ThinkingLevel, string> = {
  off: "关",
  low: "低",
  medium: "中",
  high: "高",
};

/**
 * P-MULTI-API — input-area control adopted from openhanako's
 * `ThinkingLevelButton` (`desktop/src/react/components/InputArea.tsx`).
 * Cycles `off → low → medium → high → off` on click; tooltip explains
 * the next state. Persists to `localStorage[IF2AI_THINKING_LEVEL]` and
 * is consumed by `build_chat_completion_request_for_provider` on the
 * Rust side via the `IF2AI_THINKING_LEVEL` env var bridge.
 */
export function ThinkingLevelButton({ visible, className }: ThinkingLevelButtonProps) {
  const [level, setLevel] = useThinkingLevel();
  if (!visible) return null;
  const next = ORDER[(ORDER.indexOf(level) + 1) % ORDER.length];
  return (
    <button
      type="button"
      title={`当前：${LABEL[level]}（点击切换为「${LABEL[next]}」）`}
      onClick={() => setLevel(next)}
      className={cn(
        "inline-flex items-center gap-1 rounded-md border border-border/50 bg-surface px-2 py-1 text-[11px] leading-none text-muted-foreground transition-colors hover:border-border hover:text-foreground",
        className,
      )}
    >
      <Zap className="h-3 w-3" />
      推理: {SHORT[level]}
    </button>
  );
}
