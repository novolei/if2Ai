// MIG-012 — window / shell domain facade.
//
// Chat prefill event + settings window open. Narrow surface so
// App.tsx stops knowing the underlying Tauri command / event
// literal names.

import type { UnlistenFn } from '@tauri-apps/api/event'

import { getApiClient } from './client.ts'

/** Payload shape of the `if2ai-chat-prefill` cross-window event. */
export interface ChatPrefillPayload {
  prompt: string
}

/** Event channel name the backend emits chat-prefill requests on. */
const CHAT_PREFILL_EVENT = 'if2ai-chat-prefill'

/** Subscribe to chat-prefill events. Returns the unlisten
 * function; callers MUST invoke it on cleanup. */
export async function listenToChatPrefill(
  handler: (payload: ChatPrefillPayload) => void,
): Promise<UnlistenFn> {
  return getApiClient().subscribe<ChatPrefillPayload>(CHAT_PREFILL_EVENT, (event) => {
    handler(event.payload)
  })
}

/** Open the settings window. */
export async function openSettingsWindow(): Promise<void> {
  return getApiClient().call<void>('open_settings_window')
}
