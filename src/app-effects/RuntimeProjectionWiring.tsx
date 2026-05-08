import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { wireRuntimeProjectionListeners } from "@/runtime-projection";
import { evolutionEventStore } from "@/state";
import { getActiveModel } from "@/api";
import { useAutoCompactToast } from "./useAutoCompactToast";

export interface RuntimeProjectionWiringProps {
  /**
   * Called when a model becomes active (initial mount and whenever
   * `if2ai:models-changed` / `if2ai://models-changed` fires). Receives
   * a "<provider_id>/<model_id>" composite key, ready for the chat-ui
   * dropdown.
   */
  onActiveModelChanged: (modelKey: string) => void;
}

/**
 * Pure effect container that owns mount-time runtime/projection wiring
 * extracted from `App.tsx` (GF-03 PR-1):
 *  - `wireRuntimeProjectionListeners()` — canonical projection bridge.
 *  - `runtime_event` envelope listener that pipes evolution events into
 *    `evolutionEventStore`.
 *  - Active model refresh on `if2ai:models-changed` (window) and
 *    `if2ai://models-changed` (Tauri).
 *  - `chat_compact_completed` toast subscription (auto-compact UX).
 *
 * Renders nothing.
 */
export function RuntimeProjectionWiring({
  onActiveModelChanged,
}: RuntimeProjectionWiringProps): null {
  // Projection bridge — single line.
  useEffect(() => {
    const unwire = wireRuntimeProjectionListeners();
    return () => unwire();
  }, []);

  // Models-changed + initial active model refresh.
  useEffect(() => {
    let cancelled = false;
    let unlistenModelsChanged: (() => void) | null = null;
    const refreshActiveModel = async () => {
      try {
        const activeModel = await getActiveModel();
        if (cancelled) return;
        if (activeModel) {
          onActiveModelChanged(
            `${activeModel.provider_id}/${activeModel.model_id}`,
          );
        }
      } catch {
        // Fallback: leave empty so chat-ui shows first available model.
      }
    };
    void refreshActiveModel();
    const onModelsChanged = () => {
      void refreshActiveModel();
    };
    window.addEventListener("if2ai:models-changed", onModelsChanged);
    void listen("if2ai://models-changed", onModelsChanged).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenModelsChanged = unlisten;
    });
    return () => {
      cancelled = true;
      window.removeEventListener("if2ai:models-changed", onModelsChanged);
      unlistenModelsChanged?.();
    };
  }, [onActiveModelChanged]);

  // WU-001 — Agent Evolution `runtime_event` envelope listener.
  useEffect(() => {
    let unlistenEvolution: (() => void) | null = null;
    let cancelled = false;
    void listen<unknown>("runtime_event", (event) => {
      try {
        evolutionEventStore.applyEnvelope(event.payload);
      } catch (err) {
        console.warn("[evolution_emitter] applyEnvelope failed", err);
      }
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenEvolution = unlisten;
    });
    return () => {
      cancelled = true;
      unlistenEvolution?.();
    };
  }, []);

  // Auto-compact toast (chat_compact_completed).
  useAutoCompactToast();

  return null;
}
