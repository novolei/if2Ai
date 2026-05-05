import { useCallback, useMemo } from "react";
import { respondPermission } from "@/api";
import {
  runtimeProjectionStore,
  useRuntimeProjectionSelector,
} from "@/runtime-projection";
import type { PermissionRequestPayload } from "@/transport/contracts";

/** Decision values forwarded to `respondPermission`. */
export type PermissionDecision = "allow" | "deny";
/** Scope values forwarded to `respondPermission`. */
export type PermissionScope = "once" | "session";

/**
 * Return shape of the permission overlay hook. `permissionPrompt`
 * is `null` when no approval is pending; otherwise it is the first
 * pending approval projected into the legacy
 * `PermissionRequestPayload` shape consumed by the dialog.
 */
export interface UsePermissionOverlayResult {
  permissionPrompt: PermissionRequestPayload | null;
  decide: (
    decision: PermissionDecision,
    scope?: PermissionScope,
  ) => Promise<void>;
}

/**
 * Subscribes to `runtimeProjectionStore.approvals`, picks the first
 * pending approval (single-prompt UX), and exposes a `decide` action
 * that forwards to the backend via `respondPermission` and dispatches
 * `permission_resolved` so the projection reducer clears the entry.
 *
 * Extracted from `App.tsx` (GF-03 PR-5). Behavior is identical to
 * the previous inline computation + `handlePermissionDecision`
 * handler — the dialog still closes only after the projection
 * `permission_resolved` dispatch fires (in the `finally` block).
 */
export function usePermissionOverlay(): UsePermissionOverlayResult {
  const approvals = useRuntimeProjectionSelector((s) => s.approvals);
  const permissionPrompt = useMemo<PermissionRequestPayload | null>(() => {
    const ids = Object.keys(approvals);
    if (ids.length === 0) return null;
    const a = approvals[ids[0]];
    return {
      session_id: a.sessionId,
      tool_name: a.toolName,
      permission_mode: a.permissionMode,
      current_mode: a.currentMode,
      message: a.message,
    };
  }, [approvals]);

  const decide = useCallback(
    async (
      decision: PermissionDecision,
      scope: PermissionScope = "once",
    ): Promise<void> => {
      if (!permissionPrompt) return;
      const sessionId = permissionPrompt.session_id;
      try {
        await respondPermission(sessionId, decision, {
          toolName: permissionPrompt.tool_name,
          scope,
        });
      } catch (err) {
        console.error("Failed to respond permission:", err);
      } finally {
        runtimeProjectionStore.dispatch({
          kind: "permission_resolved",
          sessionId,
          decision,
          scope,
          receivedAt: Date.now(),
        });
      }
    },
    [permissionPrompt],
  );

  return { permissionPrompt, decide };
}
