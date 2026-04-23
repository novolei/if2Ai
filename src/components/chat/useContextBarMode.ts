import { useEffect, useState } from "react";

/**
 * ContextBar render mode (P2-11 / UX follow-up).
 *
 * - `full`: legacy Steward-style 5-segment bar (system / memory / history /
 *   output_reserve + remaining). Useful for "下一轮请求会怎么花" planning.
 * - `session-only`: collapse system / memory / output_reserve into a single
 *   muted "基线" segment and let the session-local `history_tokens` segment
 *   dominate the bar so per-session differences are immediately obvious.
 */
export type ContextBarMode = "full" | "session-only";

const STORAGE_KEY = "IF2AI_CONTEXT_BAR_MODE";
const DEFAULT_MODE: ContextBarMode = "full";

function readStored(): ContextBarMode {
  if (typeof window === "undefined") return DEFAULT_MODE;
  const raw = window.localStorage.getItem(STORAGE_KEY);
  return raw === "session-only" || raw === "full" ? raw : DEFAULT_MODE;
}

/**
 * Hook + setter wrapping `localStorage[IF2AI_CONTEXT_BAR_MODE]`. Cross-tab
 * `storage` event sync keeps multiple Tauri windows in agreement.
 */
export function useContextBarMode(): [ContextBarMode, (next: ContextBarMode) => void] {
  const [mode, setMode] = useState<ContextBarMode>(readStored);

  useEffect(() => {
    const handler = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) setMode(readStored());
    };
    window.addEventListener("storage", handler);
    return () => window.removeEventListener("storage", handler);
  }, []);

  const update = (next: ContextBarMode) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, next);
    }
    setMode(next);
    // Fire a synthetic event for same-tab listeners (storage event only
    // fires across tabs).
    if (typeof window !== "undefined") {
      window.dispatchEvent(new StorageEvent("storage", { key: STORAGE_KEY, newValue: next }));
    }
  };

  return [mode, update];
}
