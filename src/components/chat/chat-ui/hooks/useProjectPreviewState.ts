/**
 * GF-01 PR-08 — extracted from `src/components/ui/chat-ui.tsx`. Absorbs
 * the preview state machine that PR-07 explicitly deferred to this PR
 * (see the JSDoc on `panels/ProjectPreviewMount.tsx`).
 *
 * Owns the entire project-preview tab machine: open tabs + active tab
 * pointer + per-tab edit drafts + per-tab save state + dirty-path set
 * + autosave timers, plus every callback (`openPreviewTab`,
 * `closePreviewTab`, `closePreviewPanel`, `refreshPreviewTab`,
 * `handlePreviewDraftChange`, `savePreviewDraft`,
 * `schedulePreviewAutosave`) and the `messages → file_write` auto-
 * refresh effect.
 *
 * Behaviour is render-equivalent — bodies preserved verbatim from the
 * original inline definitions; the only structural change is moving
 * them into a hook so the chat-ui shell can import a single bundle.
 */

import * as React from 'react'

import {
  readFilePreview,
  writeFileContents,
  type FilePreviewPayload,
} from '@/lib/tauri'
import type { Message } from '@/components/ui/chat-ui'

/** Autosave debounce window after the last keystroke. Preserved
 * verbatim from the original chat-ui constant. */
export const PREVIEW_AUTOSAVE_DELAY_MS = 900

/** Per-tab save lifecycle state. */
export type PreviewSaveState = 'idle' | 'saving' | 'saved' | 'error'

/** Bag returned by `useProjectPreviewState`. The fields are named to
 * match the props on `<ProjectPreviewMount>` plus the internal flags
 * the chat-ui shell consults (e.g. `isProjectPreviewOpen` drives the
 * focus-mode grid layout). */
export interface ProjectPreviewState {
  projectPreviewTabs: FilePreviewPayload[]
  setProjectPreviewTabs: React.Dispatch<React.SetStateAction<FilePreviewPayload[]>>
  activeProjectPreviewPath: string | null
  setActiveProjectPreviewPath: React.Dispatch<React.SetStateAction<string | null>>
  isProjectPreviewOpen: boolean
  setIsProjectPreviewOpen: React.Dispatch<React.SetStateAction<boolean>>
  projectPreviewDrafts: Record<string, string>
  setProjectPreviewDrafts: React.Dispatch<React.SetStateAction<Record<string, string>>>
  projectPreviewSaveStates: Record<string, PreviewSaveState>
  setProjectPreviewSaveStates: React.Dispatch<React.SetStateAction<Record<string, PreviewSaveState>>>
  projectPreviewDirtyPaths: string[]
  setProjectPreviewDirtyPaths: React.Dispatch<React.SetStateAction<string[]>>
  /** Pending autosave timers, keyed by absolute file path. Exposed so
   * the shell-level cleanup effect can call `clearTimeout` on unmount
   * (matches the original chat-ui behaviour). */
  projectPreviewSaveTimersRef: React.MutableRefObject<Record<string, ReturnType<typeof setTimeout>>>
  /** Open / focus a preview tab. Idempotent on the same path:
   * subsequent calls update the tab's payload in place and do NOT
   * stomp the in-progress draft. */
  openPreviewTab: (preview: FilePreviewPayload) => void
  /** Close one tab. Flushes any pending draft synchronously via
   * `savePreviewDraft` first, then drops the tab + collapses the
   * panel if no tabs remain. */
  closePreviewTab: (path: string) => void
  /** Close the entire panel. Flushes every dirty draft first. */
  closePreviewPanel: () => void
  /** Persist the latest draft for a single path immediately (used by
   * the panel's "save now" button + close-tab flush). */
  savePreviewDraft: (path: string, content: string) => Promise<void>
  /** Replace the on-disk content for an open tab without disturbing
   * the user's in-flight draft. Called by the file_write auto-refresh
   * effect and by the panel's manual "refresh" button. */
  refreshPreviewTab: (absPath: string) => Promise<void>
  /** Editor onChange handler — writes draft, marks dirty, kicks the
   * autosave debounce timer. */
  handlePreviewDraftChange: (path: string, value: string) => void
  /** Imperative reset, called by the chat-ui shell when the active
   * workdir changes. Clears every tab + draft + timer. */
  resetAll: () => void
}

/** Centralise the preview state machine. Caller injects the
 * `refreshDirectoryPreview` helper (owned by the rail) so we can
 * trigger a silent rail refresh after a successful save without the
 * hook needing to know how the rail loads its data. `messages` /
 * `defaultWorkdir` power the file_write→refresh effect.
 * `onProjectRailOpenChange` re-opens the rail when the panel closes
 * so the user doesn't lose navigation context. */
export function useProjectPreviewState(input: {
  messages: Message[]
  defaultWorkdir?: string
  onProjectRailOpenChange?: React.Dispatch<React.SetStateAction<boolean>>
  refreshDirectoryPreview: (options?: { silent?: boolean }) => Promise<void> | void
}): ProjectPreviewState {
  const { messages, defaultWorkdir, onProjectRailOpenChange, refreshDirectoryPreview } = input

  const [projectPreviewTabs, setProjectPreviewTabs] = React.useState<FilePreviewPayload[]>([])
  const [activeProjectPreviewPath, setActiveProjectPreviewPath] = React.useState<string | null>(null)
  const [isProjectPreviewOpen, setIsProjectPreviewOpen] = React.useState(false)
  const [projectPreviewDrafts, setProjectPreviewDrafts] = React.useState<Record<string, string>>({})
  const [projectPreviewSaveStates, setProjectPreviewSaveStates] = React.useState<Record<string, PreviewSaveState>>({})
  const [projectPreviewDirtyPaths, setProjectPreviewDirtyPaths] = React.useState<string[]>([])
  const projectPreviewSaveTimersRef = React.useRef<Record<string, ReturnType<typeof setTimeout>>>({})

  const savePreviewDraft = React.useCallback(async (path: string, content: string) => {
    setProjectPreviewSaveStates((current) => ({ ...current, [path]: 'saving' }))
    try {
      await writeFileContents(path, content)
      setProjectPreviewTabs((current) =>
        current.map((item) => (item.path === path ? { ...item, content } : item)),
      )
      setProjectPreviewDirtyPaths((current) => current.filter((item) => item !== path))
      setProjectPreviewSaveStates((current) => ({ ...current, [path]: 'saved' }))
      window.setTimeout(() => {
        setProjectPreviewSaveStates((current) =>
          current[path] === 'saved' ? { ...current, [path]: 'idle' } : current,
        )
      }, 1200)
      void refreshDirectoryPreview({ silent: true })
    } catch (err) {
      console.error('Failed to save preview draft:', err)
      setProjectPreviewSaveStates((current) => ({ ...current, [path]: 'error' }))
    }
  }, [refreshDirectoryPreview])

  const schedulePreviewAutosave = React.useCallback((path: string, content: string) => {
    const timers = projectPreviewSaveTimersRef.current
    if (timers[path]) {
      clearTimeout(timers[path])
    }
    timers[path] = setTimeout(() => {
      delete timers[path]
      void savePreviewDraft(path, content)
    }, PREVIEW_AUTOSAVE_DELAY_MS)
  }, [savePreviewDraft])

  const openPreviewTab = React.useCallback((preview: FilePreviewPayload) => {
    setProjectPreviewTabs((current) =>
      current.some((item) => item.path === preview.path)
        ? current.map((item) => (item.path === preview.path ? preview : item))
        : [...current, preview],
    )
    setActiveProjectPreviewPath(preview.path)
    setIsProjectPreviewOpen(true)
    setProjectPreviewDrafts((current) =>
      current[preview.path] !== undefined ? current : { ...current, [preview.path]: preview.content ?? '' },
    )
    setProjectPreviewSaveStates((current) => ({ ...current, [preview.path]: current[preview.path] ?? 'idle' }))
  }, [])

  const refreshPreviewTab = React.useCallback(async (absPath: string) => {
    try {
      const fresh = await readFilePreview(absPath)
      setProjectPreviewTabs((current) => {
        if (!current.some((item) => item.path === fresh.path)) return current
        return current.map((item) => (item.path === fresh.path ? fresh : item))
      })
      // Only sync draft when user isn't editing — don't silently overwrite.
      setProjectPreviewDrafts((current) => {
        if (projectPreviewDirtyPaths.includes(fresh.path)) return current
        return { ...current, [fresh.path]: fresh.content ?? '' }
      })
    } catch (err) {
      console.warn('[preview] refresh failed for', absPath, err)
    }
  }, [projectPreviewDirtyPaths])

  // Auto-refresh open tabs whenever a `file_write` tool call completes
  // for one of their paths.
  const processedToolIdsRef = React.useRef<Set<string>>(new Set())
  React.useEffect(() => {
    if (projectPreviewTabs.length === 0) return
    for (const message of messages) {
      if (message.role !== 'tool') continue
      if (message.toolStatus !== 'completed') continue
      const toolName = message.toolName ?? ''
      if (!toolName.includes('file_write')) continue
      const toolCallId = message.toolCallId
      if (!toolCallId || processedToolIdsRef.current.has(toolCallId)) continue
      const args = (message.toolArgs ?? {}) as Record<string, unknown>
      const rawPath = typeof args.path === 'string' ? args.path : ''
      if (!rawPath) continue
      const root = message.effectiveWorkdir ?? defaultWorkdir ?? ''
      const absCandidate = rawPath.startsWith('/')
        ? rawPath
        : root
          ? `${root.replace(/\/+$/, '')}/${rawPath.replace(/^\/+/, '')}`
          : rawPath
      const matched = projectPreviewTabs.find(
        (tab) => tab.path === absCandidate || tab.path === rawPath || tab.path.endsWith(`/${rawPath}`),
      )
      processedToolIdsRef.current.add(toolCallId)
      if (matched) {
        void refreshPreviewTab(matched.path)
      }
    }
  }, [messages, projectPreviewTabs, defaultWorkdir, refreshPreviewTab])

  const closePreviewTab = React.useCallback((path: string) => {
    const timers = projectPreviewSaveTimersRef.current
    if (timers[path]) {
      clearTimeout(timers[path])
      delete timers[path]
    }
    if (projectPreviewDirtyPaths.includes(path)) {
      const next = projectPreviewDrafts[path] ?? ''
      void savePreviewDraft(path, next)
    }
    setProjectPreviewTabs((current) => {
      const next = current.filter((item) => item.path !== path)
      setActiveProjectPreviewPath((active) => {
        if (active !== path) return active
        return next.at(-1)?.path ?? null
      })
      if (next.length === 0) {
        setIsProjectPreviewOpen(false)
        onProjectRailOpenChange?.(true)
      }
      return next
    })
    setProjectPreviewDirtyPaths((current) => current.filter((item) => item !== path))
  }, [onProjectRailOpenChange, projectPreviewDirtyPaths, projectPreviewDrafts, savePreviewDraft])

  const closePreviewPanel = React.useCallback(() => {
    Object.values(projectPreviewSaveTimersRef.current).forEach((timer) => clearTimeout(timer))
    projectPreviewSaveTimersRef.current = {}
    projectPreviewDirtyPaths.forEach((path) => {
      const next = projectPreviewDrafts[path] ?? ''
      void savePreviewDraft(path, next)
    })
    setIsProjectPreviewOpen(false)
    onProjectRailOpenChange?.(true)
  }, [onProjectRailOpenChange, projectPreviewDirtyPaths, projectPreviewDrafts, savePreviewDraft])

  const handlePreviewDraftChange = React.useCallback((path: string, value: string) => {
    setProjectPreviewDrafts((current) => ({ ...current, [path]: value }))
    setProjectPreviewDirtyPaths((current) => (current.includes(path) ? current : [...current, path]))
    setProjectPreviewSaveStates((current) => ({ ...current, [path]: 'idle' }))
    schedulePreviewAutosave(path, value)
  }, [schedulePreviewAutosave])

  const resetAll = React.useCallback(() => {
    Object.values(projectPreviewSaveTimersRef.current).forEach((timer) => clearTimeout(timer))
    projectPreviewSaveTimersRef.current = {}
    setProjectPreviewTabs([])
    setActiveProjectPreviewPath(null)
    setIsProjectPreviewOpen(false)
    setProjectPreviewDrafts({})
    setProjectPreviewSaveStates({})
    setProjectPreviewDirtyPaths([])
  }, [])

  // Cleanup: clear every pending autosave timer on unmount (matches
  // the original chat-ui shell-level cleanup effect).
  React.useEffect(() => {
    return () => {
      Object.values(projectPreviewSaveTimersRef.current).forEach((timer) => clearTimeout(timer))
    }
  }, [])

  return {
    projectPreviewTabs,
    setProjectPreviewTabs,
    activeProjectPreviewPath,
    setActiveProjectPreviewPath,
    isProjectPreviewOpen,
    setIsProjectPreviewOpen,
    projectPreviewDrafts,
    setProjectPreviewDrafts,
    projectPreviewSaveStates,
    setProjectPreviewSaveStates,
    projectPreviewDirtyPaths,
    setProjectPreviewDirtyPaths,
    projectPreviewSaveTimersRef,
    openPreviewTab,
    closePreviewTab,
    closePreviewPanel,
    savePreviewDraft,
    refreshPreviewTab,
    handlePreviewDraftChange,
    resetAll,
  }
}
