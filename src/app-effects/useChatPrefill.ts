import { useEffect } from "react";
import { listenToChatPrefill } from "@/api";
import type { AppSection } from "@/modules/app-shell/types";

export interface UseChatPrefillArgs {
  /**
   * Switches the active app-shell section. The hook always forces
   * `"chat"` when a prefill arrives so the user lands on the chat
   * surface regardless of where they were.
   */
  setActiveSection: (section: AppSection) => void;
  /**
   * Receives the prefill prompt text. Only invoked when the payload
   * carries a non-empty trimmed `prompt` string.
   */
  setInput: (text: string) => void;
}

/**
 * Subscribes to the `if2ai-chat-prefill` Tauri event so external
 * surfaces (tray, deeplink, system shortcut) can deliver a prompt
 * directly into the main chat input. Extracted from `App.tsx`
 * (GF-03 PR-1).
 */
export function useChatPrefill({
  setActiveSection,
  setInput,
}: UseChatPrefillArgs): void {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenToChatPrefill((payload) => {
      setActiveSection("chat");
      if (typeof payload?.prompt === "string" && payload.prompt.trim()) {
        setInput(payload.prompt);
      }
    })
      .then((dispose) => {
        unlisten = dispose;
      })
      .catch((err) => {
        console.error("Failed to listen chat prefill event:", err);
      });
    return () => {
      if (unlisten) unlisten();
    };
    // setActiveSection / setInput are stable React setters in the
    // App.tsx call site (useState dispatcher), so [] preserves the
    // original mount/unmount semantics from before extraction.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
