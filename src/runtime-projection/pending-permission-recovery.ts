// Pending permission recovery fetch seam.
//
// The live `permission-request` event remains the fast path. This helper
// rehydrates the same canonical projection after refresh/reconnect by asking
// the backend for the current pending permission record.

import { getPendingPermission } from "@/api/streaming";

import { translatePermissionRequestPayload } from "./runtime-event-translator.ts";
import {
  runtimeProjectionStore,
  type RuntimeProjectionStore,
} from "./runtime-projection-store.ts";

/** Recover and project the current pending permission for one session. */
export async function refreshPendingPermission(
  sessionId: string,
  store: RuntimeProjectionStore = runtimeProjectionStore,
): Promise<boolean> {
  const pending = await getPendingPermission(sessionId);
  if (!pending) {
    return false;
  }
  store.dispatch(translatePermissionRequestPayload(pending));
  return true;
}
