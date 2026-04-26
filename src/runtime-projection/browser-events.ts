import type { BrowserStatusEvent } from '@/lib/tauri'

import type {
  SmartBrowserBackend,
  SmartBrowserEscalationState,
  SmartBrowserProjectionEvent,
} from './types.ts'

const DEFAULT_BACKEND: SmartBrowserBackend = 'local_rust_cdp'
const DEFAULT_ESCALATION: SmartBrowserEscalationState = 'none'

function normalizeBackend(value: BrowserStatusEvent['backend']): SmartBrowserBackend {
  if (
    value === 'local_rust_cdp' ||
    value === 'browser_use_mcp' ||
    value === 'browser_use_cloud'
  ) {
    return value
  }
  return DEFAULT_BACKEND
}

function normalizeEscalation(
  value: BrowserStatusEvent['escalation_state'],
): SmartBrowserEscalationState {
  if (
    value === 'none' ||
    value === 'suggested' ||
    value === 'approval_required' ||
    value === 'approved' ||
    value === 'active' ||
    value === 'blocked'
  ) {
    return value
  }
  return DEFAULT_ESCALATION
}

export function translateBrowserStatusPayload(
  payload: BrowserStatusEvent,
  receivedAt = Date.now(),
): SmartBrowserProjectionEvent {
  return {
    kind: 'smart_browser_projection',
    sessionId: payload.session_id,
    smartBrowserSessionId: payload.session_id,
    backend: normalizeBackend(payload.backend),
    running: payload.running,
    url: payload.url,
    title: payload.title ?? null,
    thumbnail: payload.thumbnail,
    takenOver: payload.taken_over ?? false,
    lastAction: payload.last_action ?? null,
    diagnostics: {
      downloads: payload.downloads_count ?? 0,
      console: payload.console_count ?? 0,
      networkErrors: payload.network_error_count ?? 0,
    },
    escalationState: normalizeEscalation(payload.escalation_state),
    receivedAt,
  }
}
