/**
 * Single source of truth for "which TTS profile is currently active"
 * on the chat side.
 *
 * Storage: `localStorage[if2ai.tts.activeProfileId]`.  Cross-window
 * sync goes through `cross:tts-active-profile-changed`.
 *
 * Resolution rule: if no profile id is stored OR the stored id no
 * longer exists, fall back to the backend's `default_profile_id`.
 * The chat-side hooks (`useAgentVoiceBridge`, `MessageVoiceButton`)
 * call `resolveActiveProfile()` to perform that fallback so they
 * never crash on a stale id.
 */
import {
  listTtsProfiles,
  ttsParamsFromSettings,
  ttsSettingsFromProfile,
  type TtsGenerationParams,
  type TtsProfile,
} from '@/lib/tauri'
import { broadcastChange } from '@/lib/crossWindowSync'

const ACTIVE_PROFILE_LOCAL_STORAGE_KEY = 'if2ai.tts.activeProfileId'

export function getActiveProfileId(): string | null {
  try {
    return localStorage.getItem(ACTIVE_PROFILE_LOCAL_STORAGE_KEY)
  } catch {
    return null
  }
}

export function setActiveProfileId(id: string | null): void {
  try {
    if (id == null || id === '') {
      localStorage.removeItem(ACTIVE_PROFILE_LOCAL_STORAGE_KEY)
    } else {
      localStorage.setItem(ACTIVE_PROFILE_LOCAL_STORAGE_KEY, id)
    }
  } catch {
    /* ignore */
  }
  void broadcastChange('cross:tts-active-profile-changed', { id })
}

/**
 * Resolved chat-side TTS bundle: the active profile + the
 * `TtsGenerationParams` derived from it (ready to pass straight into
 * `ttsStreamStart`).
 */
export interface ResolvedActiveProfile {
  profile: TtsProfile
  params: TtsGenerationParams
}

/**
 * Fetch the latest profile book and return the resolved active
 * profile.  Falls back to backend default; if even that is missing,
 * returns `null`.
 */
export async function resolveActiveProfile(): Promise<ResolvedActiveProfile | null> {
  let book: Awaited<ReturnType<typeof listTtsProfiles>>
  try {
    book = await listTtsProfiles()
  } catch (err) {
    console.warn('[active-profile] listTtsProfiles failed:', err)
    return null
  }
  const wantId = getActiveProfileId()
  const found =
    (wantId ? book.profiles.find((p) => p.id === wantId) : undefined) ??
    book.profiles.find((p) => p.id === book.default_profile_id) ??
    book.profiles[0]
  if (!found) return null
  return {
    profile: found,
    params: ttsParamsFromSettings(ttsSettingsFromProfile(found)),
  }
}
