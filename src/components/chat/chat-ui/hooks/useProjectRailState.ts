/**
 * GF-01 PR-08 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Owns every piece of "project files rail" state the chat shell used
 * to inline: width persistence (with min/max clamp), the path /
 * breadcrumbs / expanded-paths bookkeeping, child-tree map, loading
 * flags, sort mode, the transient preview-error notice with a
 * self-clearing timer, and a refresh sequence counter so out-of-order
 * directory fetches can be discarded.
 *
 * Render-equivalent move: bodies preserved verbatim from the original
 * inline definitions; only function shape changed (now returned from a
 * hook instead of declared at component-scope).
 */

import * as React from 'react'

import type { DirectoryEntryPreview } from '@/lib/tauri'
import {
  PROJECT_RAIL_MAX_WIDTH,
  PROJECT_RAIL_MIN_WIDTH,
} from '@/components/chat/chat-ui/sidebars/utils'

/** localStorage key holding the persisted rail width in pixels. */
export const PROJECT_RAIL_WIDTH_STORAGE_KEY = 'projectRailWidthV1'
/** How long the transient "couldn't preview that file" notice sticks
 * around (ms) before auto-clearing itself. */
export const PROJECT_RAIL_NOTICE_DURATION_MS = 2800
/** Default rail width on a fresh install. */
export const PROJECT_RAIL_DEFAULT_WIDTH = 200

/** Map keyed by absolute folder path → its child entries. */
export type RailTreeMap = Record<string, DirectoryEntryPreview[]>

/** Single breadcrumb segment in the rail header navigation. */
export interface RailBreadcrumb {
  label: string
  path: string
}

/** Full bag of rail state the chat-ui shell consumes. Refs are
 * exposed so call-sites that previously bumped `projectRailRefreshSeqRef.current`
 * directly can keep doing so without behavioural drift. */
export interface ProjectRailState {
  /** Persisted rail width in pixels, clamped to [MIN, MAX]. */
  projectRailWidth: number
  setProjectRailWidth: React.Dispatch<React.SetStateAction<number>>
  projectRailEntries: DirectoryEntryPreview[]
  setProjectRailEntries: React.Dispatch<React.SetStateAction<DirectoryEntryPreview[]>>
  projectRailTree: RailTreeMap
  setProjectRailTree: React.Dispatch<React.SetStateAction<RailTreeMap>>
  projectRailExpandedPaths: string[]
  setProjectRailExpandedPaths: React.Dispatch<React.SetStateAction<string[]>>
  projectRailLoadingPaths: string[]
  setProjectRailLoadingPaths: React.Dispatch<React.SetStateAction<string[]>>
  isProjectRailLoading: boolean
  setIsProjectRailLoading: React.Dispatch<React.SetStateAction<boolean>>
  projectRailSort: 'recent' | 'name'
  setProjectRailSort: React.Dispatch<React.SetStateAction<'recent' | 'name'>>
  projectRailPath: string | null
  setProjectRailPath: React.Dispatch<React.SetStateAction<string | null>>
  projectRailPreviewError: string | null
  setProjectRailPreviewError: React.Dispatch<React.SetStateAction<string | null>>
  /** Monotonic refresh sequence — used by callers to discard
   * out-of-order async directory loads. Bump on every refresh start. */
  projectRailRefreshSeqRef: React.MutableRefObject<number>
  /** Pending notice-clear timer; the hook owns lifecycle but exposes
   * the ref so the shell's reset effect can cancel it on workdir swap. */
  projectRailNoticeTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
  /** Show / clear a transient preview-error notice. Pass `null` to
   * clear immediately; pass a string to schedule auto-clear after
   * `PROJECT_RAIL_NOTICE_DURATION_MS`. */
  showProjectRailNotice: (message: string | null) => void
  /** Persist + clamp helper exposed for the rail's resize handle. */
  applyRailWidth: (next: number) => void
}

/** Initial-width reader: clamps any persisted value to the safety
 * range and falls back to the default on missing / NaN entries. */
function readInitialRailWidth(): number {
  if (typeof window === 'undefined') return PROJECT_RAIL_DEFAULT_WIDTH
  const stored = Number(window.localStorage.getItem(PROJECT_RAIL_WIDTH_STORAGE_KEY))
  if (Number.isFinite(stored)) {
    return Math.min(PROJECT_RAIL_MAX_WIDTH, Math.max(PROJECT_RAIL_MIN_WIDTH, stored))
  }
  return PROJECT_RAIL_DEFAULT_WIDTH
}

/** Centralise the project-files rail state previously inlined inside
 * `ChatUI`. Caller passes the active `defaultWorkdir` so the path
 * defaults reset when the user switches projects (matching the
 * pre-existing reset effect). */
export function useProjectRailState(input: {
  defaultWorkdir?: string
}): ProjectRailState {
  const { defaultWorkdir } = input

  const [projectRailEntries, setProjectRailEntries] = React.useState<DirectoryEntryPreview[]>([])
  const [projectRailTree, setProjectRailTree] = React.useState<RailTreeMap>({})
  const [projectRailExpandedPaths, setProjectRailExpandedPaths] = React.useState<string[]>([])
  const [projectRailLoadingPaths, setProjectRailLoadingPaths] = React.useState<string[]>([])
  const [isProjectRailLoading, setIsProjectRailLoading] = React.useState(false)
  const [projectRailSort, setProjectRailSort] = React.useState<'recent' | 'name'>('recent')
  const [projectRailPath, setProjectRailPath] = React.useState<string | null>(defaultWorkdir ?? null)
  const [projectRailPreviewError, setProjectRailPreviewError] = React.useState<string | null>(null)
  const [projectRailWidth, setProjectRailWidth] = React.useState<number>(readInitialRailWidth)

  const projectRailRefreshSeqRef = React.useRef(0)
  const projectRailNoticeTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)

  // Persist rail width whenever it changes.
  React.useEffect(() => {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(PROJECT_RAIL_WIDTH_STORAGE_KEY, String(projectRailWidth))
  }, [projectRailWidth])

  const showProjectRailNotice = React.useCallback((message: string | null) => {
    if (projectRailNoticeTimerRef.current) {
      clearTimeout(projectRailNoticeTimerRef.current)
      projectRailNoticeTimerRef.current = null
    }
    setProjectRailPreviewError(message)
    if (message) {
      projectRailNoticeTimerRef.current = setTimeout(() => {
        setProjectRailPreviewError((current) => (current === message ? null : current))
        projectRailNoticeTimerRef.current = null
      }, PROJECT_RAIL_NOTICE_DURATION_MS)
    }
  }, [])

  const applyRailWidth = React.useCallback((next: number) => {
    const clamped = Math.min(PROJECT_RAIL_MAX_WIDTH, Math.max(PROJECT_RAIL_MIN_WIDTH, next))
    setProjectRailWidth(clamped)
  }, [])

  // Cleanup: cancel any pending notice timer on unmount.
  React.useEffect(() => {
    return () => {
      if (projectRailNoticeTimerRef.current) {
        clearTimeout(projectRailNoticeTimerRef.current)
        projectRailNoticeTimerRef.current = null
      }
    }
  }, [])

  return {
    projectRailWidth,
    setProjectRailWidth,
    projectRailEntries,
    setProjectRailEntries,
    projectRailTree,
    setProjectRailTree,
    projectRailExpandedPaths,
    setProjectRailExpandedPaths,
    projectRailLoadingPaths,
    setProjectRailLoadingPaths,
    isProjectRailLoading,
    setIsProjectRailLoading,
    projectRailSort,
    setProjectRailSort,
    projectRailPath,
    setProjectRailPath,
    projectRailPreviewError,
    setProjectRailPreviewError,
    projectRailRefreshSeqRef,
    projectRailNoticeTimerRef,
    showProjectRailNotice,
    applyRailWidth,
  }
}
