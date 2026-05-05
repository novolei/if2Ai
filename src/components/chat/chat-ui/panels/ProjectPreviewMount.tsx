/**
 * GF-01 PR-07 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Thin presentational mount around `@/components/ui/ProjectPreviewPanel`.
 *
 * Scope note (deferred): the preview state machine itself
 * (`projectPreviewTabs` / `drafts` / `dirtyPaths` / `saveStates` /
 * `projectPreviewSaveTimersRef` + `savePreviewDraft` /
 * `schedulePreviewAutosave` / `openPreviewTab` / `closePreviewTab` /
 * `closePreviewPanel` / `refreshPreviewTab` /
 * `handlePreviewDraftChange` and the file_write→refresh effect with
 * its 900ms `PREVIEW_AUTOSAVE_DELAY_MS` debounce) stays in
 * `ChatUI` for this PR. Inlining the machine here would either deeply
 * couple this mount to `refreshDirectoryPreview`, `messages`,
 * `defaultWorkdir` and `onProjectRailOpenChange` (defeating the
 * extraction goal) or require the new `useProjectPreviewState` hook
 * planned for PR-08. The mount keeps the props surface intact so the
 * follow-up hook drop-in is mechanical.
 *
 * Render-equivalent move from the original inline JSX.
 */

import * as React from "react"
import { ProjectPreviewPanel } from "@/components/ui/ProjectPreviewPanel"
import type { FilePreviewPayload } from "@/lib/tauri"

/**
 * Mount the right-side project preview panel when at least one tab is
 * open. All callbacks are forwarded verbatim to the underlying
 * `ProjectPreviewPanel`; the panel owns its own internal layout, so
 * this wrapper adds no extra chrome.
 */
export function ProjectPreviewMount({
  tabs,
  activeTabPath,
  drafts,
  saveStates,
  dirtyPaths,
  onSelectTab,
  onCloseTab,
  onClosePanel,
  onOpenExternally,
  onQuoteIntoChat,
  onInsertIntoChat,
  onChangeDraft,
  onSaveNow,
  onRefresh,
}: {
  tabs: FilePreviewPayload[]
  activeTabPath: string | null
  drafts: Record<string, string>
  saveStates: Record<string, 'idle' | 'saving' | 'saved' | 'error'>
  dirtyPaths: string[]
  onSelectTab: (path: string) => void
  onCloseTab: (path: string) => void
  onClosePanel: () => void
  onOpenExternally: (preview: FilePreviewPayload) => void
  onQuoteIntoChat: (preview: FilePreviewPayload) => void
  onInsertIntoChat: (preview: FilePreviewPayload) => void
  onChangeDraft: (path: string, value: string) => void
  onSaveNow: (path: string) => void
  onRefresh: (absPath: string) => void | Promise<void>
}) {
  return (
    <ProjectPreviewPanel
      tabs={tabs}
      activeTabPath={activeTabPath}
      drafts={drafts}
      saveStates={saveStates}
      dirtyPaths={dirtyPaths}
      onSelectTab={onSelectTab}
      onCloseTab={onCloseTab}
      onClosePanel={onClosePanel}
      onOpenExternally={onOpenExternally}
      onQuoteIntoChat={onQuoteIntoChat}
      onInsertIntoChat={onInsertIntoChat}
      onChangeDraft={onChangeDraft}
      onSaveNow={onSaveNow}
      onRefresh={onRefresh}
    />
  )
}
