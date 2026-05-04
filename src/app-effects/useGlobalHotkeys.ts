import { useEffect, useState } from "react";
import { openSettingsWindow } from "@/api";

export interface UseGlobalHotkeysResult {
  isTelemetryDrawerOpen: boolean;
  setTelemetryDrawerOpen: (open: boolean) => void;
}

/**
 * Mounts the App-shell global keyboard shortcuts (GF-03 PR-1):
 *  - `⌘+,` / `Ctrl+,`        — open settings window.
 *  - `⌘+Shift+D` / `Ctrl+Shift+D` — toggle the TelemetryDrawer for the
 *    active session (Phase 6E harness observability surface).
 *
 * Owns the telemetry drawer's open/close state so the hotkey effect
 * stays self-contained; the parent passes `isTelemetryDrawerOpen` into
 * `<TelemetryDrawer open=… onClose=…>` and uses
 * `setTelemetryDrawerOpen(false)` for the explicit close button.
 */
export function useGlobalHotkeys(): UseGlobalHotkeysResult {
  // Cmd+, / Ctrl+,
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        void openSettingsWindow();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const [isTelemetryDrawerOpen, setIsTelemetryDrawerOpen] = useState(false);

  // Cmd+Shift+D / Ctrl+Shift+D
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        (e.metaKey || e.ctrlKey) &&
        e.shiftKey &&
        (e.code === "KeyD" || e.key.toLowerCase() === "d")
      ) {
        e.preventDefault();
        e.stopPropagation();
        setIsTelemetryDrawerOpen((prev) => !prev);
      }
    };
    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () =>
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
  }, []);

  return {
    isTelemetryDrawerOpen,
    setTelemetryDrawerOpen: setIsTelemetryDrawerOpen,
  };
}
