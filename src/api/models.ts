// MIG-012 / FIX-11 / ER-02 — model selection facade.
//
// Wraps the `model_get_active` / `model_set_active` / `model_list_available`
// Tauri commands so UI never imports `invoke` directly. The shape mirrors the
// backend `ModelSelection` snake_case wire (src-tauri/src/modules/config/types.rs).

import * as React from 'react'
import { getApiClient } from './client.ts'

export interface ActiveModel {
  provider_id: string
  model_id: string
  /** Auth variant for same-provider different auth configs
   *  (e.g. "moonshot-cn" vs "moonshot-code"). Absent when the
   *  stored selection doesn't carry a variant. */
  auth_variant?: string
}

/** Available model group as returned by `model_list_available`.
 * Mirrors the backend `AvailableModelGroup` snake_case wire shape. */
export interface AvailableModelGroup {
  provider_id: string
  provider_name: string
  models: AvailableModelEntry[]
}

/** Single available model entry within a provider group. */
export interface AvailableModelEntry {
  model_id: string
  name: string
  context_window?: number
  reasoning?: boolean
  reasoning_required_in_tool_calls?: boolean
  supports_reasoning_effort?: boolean
}

/** Read the user's currently selected provider/model pair, or
 * `null` when nothing has been chosen yet (fresh install). */
export async function getActiveModel(): Promise<ActiveModel | null> {
  return getApiClient().call<ActiveModel | null>('model_get_active')
}

/** Persist a new provider/model selection. Backend emits
 * `if2ai://models-changed` on success — listeners in App.tsx /
 * chat-ui pick this up to refresh dependent UI.
 *
 * `authVariant` is required for multi-auth providers (e.g. moonshot-cn
 * vs moonshot-code) where the same `providerId` is reused across
 * distinct credential sets. Omitting it is correct for single-auth
 * providers (Anthropic, OpenAI, Ollama, …). See ER-01. */
export async function setActiveModel(
  providerId: string,
  modelId: string,
  authVariant?: string,
): Promise<void> {
  return getApiClient().call<void>('model_set_active', {
    providerId,
    modelId,
    authVariant,
  })
}

/** ER-02 — one-shot fetch of the available model groups. Goes through
 * `getApiClient()` so transport swaps (sidecar / cloud) are
 * mechanical. Returns `[]` on failure so callers can render a benign
 * empty list without try/catch noise. */
export async function fetchAvailableModelGroups(): Promise<AvailableModelGroup[]> {
  try {
    return await getApiClient().call<AvailableModelGroup[]>('model_list_available')
  } catch {
    return []
  }
}

/** ER-02 — subscribe to backend "models changed" notifications without
 * importing `@tauri-apps/api` directly. Wires up both the Tauri event
 * (`if2ai://models-changed`, fired by `setActiveModel` / provider config
 * saves) and the frontend-local window event (`if2ai:models-changed`,
 * dispatched by settings pages for in-process refresh). The returned
 * disposer detaches both listeners. */
export function subscribeModelsChanged(callback: () => void): () => void {
  let unlistenTauri: (() => void) | null = null
  const onWindow = () => callback()
  if (typeof window !== 'undefined') {
    window.addEventListener('if2ai:models-changed', onWindow)
  }
  void getApiClient()
    .subscribe('if2ai://models-changed', () => callback())
    .then((unlisten) => {
      unlistenTauri = unlisten
    })
    .catch(() => {
      // Subscribe may fail in non-Tauri contexts (tests / SSR). Window
      // event keeps in-process refresh working regardless.
    })
  return () => {
    if (typeof window !== 'undefined') {
      window.removeEventListener('if2ai:models-changed', onWindow)
    }
    unlistenTauri?.()
  }
}

/** ER-02 — React hook returning the latest available model groups,
 * automatically refreshed whenever a "models changed" notification
 * fires. Internally uses `fetchAvailableModelGroups()` +
 * `subscribeModelsChanged()`, so the underlying transport is the
 * single `getApiClient()` facade.
 *
 * `loading` is true on the very first fetch and stays false during
 * subsequent invalidation refreshes (so dependent UI doesn't flash
 * spinners on every settings save). */
export function useAvailableModels(): {
  groups: AvailableModelGroup[]
  loading: boolean
  refetch: () => void
} {
  const [groups, setGroups] = React.useState<AvailableModelGroup[]>([])
  const [loading, setLoading] = React.useState(true)
  const cancelledRef = React.useRef(false)
  const initialDoneRef = React.useRef(false)

  const refetch = React.useCallback(() => {
    void (async () => {
      const next = await fetchAvailableModelGroups()
      if (cancelledRef.current) return
      setGroups(next)
      if (!initialDoneRef.current) {
        initialDoneRef.current = true
        setLoading(false)
      }
    })()
  }, [])

  React.useEffect(() => {
    cancelledRef.current = false
    refetch()
    const dispose = subscribeModelsChanged(refetch)
    return () => {
      cancelledRef.current = true
      dispose()
    }
  }, [refetch])

  return { groups, loading, refetch }
}
