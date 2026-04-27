export interface MemoryScopeEntry {
  session_id: string | null
  project_id: string | null
}

export interface MemoryScopeContext {
  activeProjectId?: string | null
  activeSessionId?: string | null
}

export type MemoryRecallVisibilityKind =
  | 'current_session'
  | 'current_project'
  | 'global'
  | 'same_project_fallback'
  | 'browser_only_unprojected_session'
  | 'browser_only_other_project'
  | 'browser_only_missing_context'

export interface MemoryScopeDiagnostic {
  kind: MemoryRecallVisibilityKind
  label: string
  title: string
  isCanonicalRecallVisible: boolean
  isFallbackCandidate: boolean
  tone: 'success' | 'info' | 'warning' | 'muted'
}

export function diagnoseMemoryScopeVisibility(
  entry: MemoryScopeEntry,
  context: MemoryScopeContext,
): MemoryScopeDiagnostic {
  if (!entry.session_id && !entry.project_id) {
    return {
      kind: 'global',
      label: '全局可召回',
      title: '全局记忆会进入当前 agent 的 scoped recall。',
      isCanonicalRecallVisible: true,
      isFallbackCandidate: false,
      tone: 'success',
    }
  }

  if (entry.session_id && context.activeSessionId === entry.session_id) {
    return {
      kind: 'current_session',
      label: '会话可召回',
      title: '这条记忆属于当前会话，会进入当前 agent 的 scoped recall。',
      isCanonicalRecallVisible: true,
      isFallbackCandidate: false,
      tone: 'success',
    }
  }

  if (
    !entry.session_id &&
    entry.project_id &&
    context.activeProjectId === entry.project_id
  ) {
    return {
      kind: 'current_project',
      label: '项目可召回',
      title: '这条记忆属于当前项目，会进入当前 agent 的 scoped recall。',
      isCanonicalRecallVisible: true,
      isFallbackCandidate: false,
      tone: 'success',
    }
  }

  if (!context.activeSessionId && entry.session_id) {
    return {
      kind: 'browser_only_missing_context',
      label: '仅浏览器',
      title: '当前未打开会话；Memory Browser 可见不代表 agent scoped recall 可见。',
      isCanonicalRecallVisible: false,
      isFallbackCandidate: false,
      tone: 'muted',
    }
  }

  if (!context.activeProjectId && entry.project_id) {
    return {
      kind: 'browser_only_missing_context',
      label: '仅浏览器',
      title: '当前未选中项目；Memory Browser 可见不代表 agent scoped recall 可见。',
      isCanonicalRecallVisible: false,
      isFallbackCandidate: false,
      tone: 'muted',
    }
  }

  if (
    entry.session_id &&
    entry.project_id &&
    context.activeProjectId === entry.project_id
  ) {
    return {
      kind: 'same_project_fallback',
      label: '同项目fallback',
      title:
        '这条记忆来自同项目旧会话；普通 scoped recall 不直接包含它，AWL-007 仅对个人/家庭/饮食问题做有界 fallback。',
      isCanonicalRecallVisible: false,
      isFallbackCandidate: true,
      tone: 'warning',
    }
  }

  if (entry.project_id && context.activeProjectId !== entry.project_id) {
    return {
      kind: 'browser_only_other_project',
      label: '仅浏览器',
      title: '这条记忆属于其他项目，不会进入当前 agent 的 scoped recall。',
      isCanonicalRecallVisible: false,
      isFallbackCandidate: false,
      tone: 'muted',
    }
  }

  return {
    kind: 'browser_only_unprojected_session',
    label: '仅浏览器',
    title: '这条记忆来自旧会话且没有项目标签，不会进入当前 agent 的 scoped recall。',
    isCanonicalRecallVisible: false,
    isFallbackCandidate: false,
    tone: 'muted',
  }
}
