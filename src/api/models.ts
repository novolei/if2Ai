// MIG-012 / FIX-11 — model selection facade.
//
// Wraps the `model_get_active` / `model_set_active` Tauri commands so
// UI never imports `invoke` directly. The shape mirrors the backend
// `ModelSelection` snake_case wire (src-tauri/src/modules/config/types.rs).

import { getApiClient } from './client.ts'

export interface ActiveModel {
  provider_id: string
  model_id: string
  /** Auth variant for same-provider different auth configs
   *  (e.g. "moonshot-cn" vs "moonshot-code"). Absent when the
   *  stored selection doesn't carry a variant. */
  auth_variant?: string
}

/** Read the user's currently selected provider/model pair, or
 * `null` when nothing has been chosen yet (fresh install). */
export async function getActiveModel(): Promise<ActiveModel | null> {
  return getApiClient().call<ActiveModel | null>('model_get_active')
}

/** Persist a new provider/model selection. Backend emits
 * `if2ai://models-changed` on success — listeners in App.tsx /
 * chat-ui pick this up to refresh dependent UI. */
export async function setActiveModel(
  providerId: string,
  modelId: string,
): Promise<void> {
  return getApiClient().call<void>('model_set_active', {
    providerId,
    modelId,
  })
}
