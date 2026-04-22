import type { PromptDiagnosticsSummary } from '@/lib/tauri'
import { broadcastChange } from '@/lib/crossWindowSync'

export interface PromptDiagnosticsSnapshot {
  sessionId: string
  projectId?: string | null
  assistantMessageId?: string | null
  updatedAt: number
  summary: PromptDiagnosticsSummary
}

const LATEST_PROMPT_DIAGNOSTICS_STORAGE_KEY = 'if2ai.latestPromptDiagnostics.v1'

function hasWindow(): boolean {
  return typeof window !== 'undefined' && typeof window.localStorage !== 'undefined'
}

export function readLatestPromptDiagnosticsSnapshot(): PromptDiagnosticsSnapshot | null {
  if (!hasWindow()) return null
  try {
    const raw = window.localStorage.getItem(LATEST_PROMPT_DIAGNOSTICS_STORAGE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as Partial<PromptDiagnosticsSnapshot> | null
    if (
      !parsed ||
      typeof parsed.sessionId !== 'string' ||
      typeof parsed.updatedAt !== 'number' ||
      !parsed.summary ||
      typeof parsed.summary.trace_id !== 'string'
    ) {
      return null
    }
    return {
      sessionId: parsed.sessionId,
      projectId: parsed.projectId ?? null,
      assistantMessageId: parsed.assistantMessageId ?? null,
      updatedAt: parsed.updatedAt,
      summary: parsed.summary,
    }
  } catch {
    return null
  }
}

export async function publishLatestPromptDiagnosticsSnapshot(
  snapshot: PromptDiagnosticsSnapshot,
): Promise<void> {
  if (hasWindow()) {
    try {
      window.localStorage.setItem(
        LATEST_PROMPT_DIAGNOSTICS_STORAGE_KEY,
        JSON.stringify(snapshot),
      )
    } catch {
      // Ignore storage quota / availability failures.
    }
  }
  await broadcastChange('cross:prompt-diagnostics-changed', snapshot)
}
