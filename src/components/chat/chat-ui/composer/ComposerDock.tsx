/**
 * GF-01 PR-06 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Composer dock: textarea + slash/@-mention overlays + drop chips +
 * bottom control row (attach, permission picker, model picker, mic,
 * send/stop) + project context bar (project / workdir / branch).
 *
 * Render-equivalent move from the legacy inline declaration:
 *   - prop interface preserved verbatim;
 *   - IME composition guard ref pattern (`isComposingRef`) preserved;
 *   - slash + @ overlay timer-ref state machine preserved;
 *   - keyboard handler is forwarded through `guardedHandleKeyDown`
 *     unchanged from the legacy implementation;
 *   - `React.memo` wrapper preserved.
 */

import * as React from "react"
import {
  ArrowUp,
  ChevronDown,
  Folder,
  FolderOpen,
  Laptop,
  Paperclip,
  Plus,
  Square,
  X,
} from "lucide-react"
import { cn } from "@/lib/utils"
import {
  listDirectoryPreview,
  type DirectoryEntryPreview,
  type PermissionMode,
} from "@/lib/tauri"
import { setActiveModel } from "@/api/models"
import { BranchPicker } from "@/components/chat/BranchPicker"
import { ModelPicker } from "@/components/chat/ModelPicker"
import { Textarea } from "@/components/ui/textarea"
import { modelItems } from "@/components/chat/chat-ui/utils/items"
import { PermissionModePicker } from "@/components/chat/chat-ui/pickers/PermissionModePicker"
import { SlashCommandSuggestions } from "@/components/chat/chat-ui/composer/SlashCommandSuggestions"
import { AtFileSuggestions } from "@/components/chat/chat-ui/composer/AtFileSuggestions"
import { type ComposerDockProps } from "@/components/chat/chat-ui/composer/ComposerDock.props"

export type { ComposerDockProps } from "@/components/chat/chat-ui/composer/ComposerDock.props"

// 语音输入按钮（SenseVoice STT）— lazy-loaded as in the original module.
const SttButtonLazy = React.lazy(() =>
  import('@/modules/chat/SttButton').then((m) => ({ default: m.SttButton }))
)

/**
 * Composer dock rendered at the bottom of the chat pane.
 */
export const ComposerDock = React.memo(function ComposerDock({
  input,
  onInputChange,
  onSubmit,
  onStop,
  isLoading,
  selectedModel,
  setSelectedModel,
  availableModelItems,
  permissionMode,
  setPermissionMode,
  selectedStrength: _selectedStrength,
  setSelectedStrength: _setSelectedStrength,
  branchLabel,
  onBranchChange,
  isGitRepo,
  onGitRepoChanged,
  isComposerFocused,
  setIsComposerFocused,
  isLeftPaneCollapsed: _isLeftPaneCollapsed,
  contentRightInset,
  contentMaxWidth,
  textareaRef,
  handleKeyDown,
  setSlashOverlay,
  slashOverlayState,
  onSlashSelect,
  slashTimerRef,
  atOverlayState,
  setAtOverlay,
  onAtSelect,
  onNavigateIntoFolder,
  onNavigateUpFolder,
  atTimerRef,
  defaultWorkdir,
  dropItems,
  onRemoveDropItem,
  onFileReferenceDrop,
  projectLabel,
  workdirLabel,
  onProjectPillClick,
}: ComposerDockProps) {
  const [isDropTarget, setIsDropTarget] = React.useState(false)

  // ── IME composition guard ────────────────────────────────────────────────
  // On macOS/Electron, pressing Enter to confirm an IME candidate fires
  // `keydown` *before* `compositionend`, and `isComposing` is already `false`
  // at that moment, so `e.nativeEvent.isComposing` alone is insufficient.
  // We track composition state with a ref and delay clearing it by one tick so
  // that the confirming Enter keydown is still intercepted.
  const isComposingRef = React.useRef(false)
  const guardedHandleKeyDown = React.useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (isComposingRef.current) return
      void handleKeyDown(e)
    },
    [handleKeyDown]
  )
  const handleInputWithSlashDetect = (value: string) => {
    // Clear pending timers
    if (slashTimerRef.current) {
      clearTimeout(slashTimerRef.current)
      slashTimerRef.current = null
    }
    if (atTimerRef.current) {
      clearTimeout(atTimerRef.current)
      atTimerRef.current = null
    }

    onInputChange(value)

    // ── Slash command detection ──────────────────────────────────────────────
    if (value.startsWith('/')) {
      const cmdPart = value.split(/\s+/)[0]
      // Dismiss once the user has typed a space (started the instruction body).
      if (value.includes(' ')) {
        setSlashOverlay(null)
      } else {
        slashTimerRef.current = setTimeout(async () => {
          try {
            const { suggestSlashCommands } = await import('@/lib/tauri')
            const suggestions = await suggestSlashCommands(cmdPart, 8)
            if (suggestions.length > 0) {
              setSlashOverlay({ visible: true, selectedIndex: 0, suggestions, rawInput: cmdPart })
            } else {
              setSlashOverlay(null)
            }
          } catch {
            setSlashOverlay(null)
          }
        }, 50)
      }
    } else {
      setSlashOverlay(null)
    }

    // ── @-mention detection ──────────────────────────────────────────────────
    // When in pinned browse mode the overlay is independent of the textarea.
    if (atOverlayState?.pinned) return

    // Find the last @ that is followed by non-space text at the end of the value.
    const atIndex = value.lastIndexOf('@')
    if (atIndex >= 0 && defaultWorkdir) {
      // Ignore when @ is immediately preceded by a word character (letter / digit / . / - / +)
      // — that pattern indicates an e-mail address (e.g. user@example.com), not a file mention.
      const charBefore = atIndex > 0 ? value[atIndex - 1] : ''
      if (/[\w.\-+]/.test(charBefore)) {
        setAtOverlay(null)
        return
      }

      const textAfterAt = value.slice(atIndex + 1)
      // Only activate when no space follows the @ (i.e. the user is still typing the query)
      if (!textAfterAt.includes(' ') && !textAfterAt.includes('\n')) {
        const query = textAfterAt
        atTimerRef.current = setTimeout(async () => {
          try {
            const entries = await listDirectoryPreview(defaultWorkdir, 48)
            const filtered = query
              ? entries.filter((e) => e.name.toLowerCase().includes(query.toLowerCase()))
              : entries
            const limited = filtered.slice(0, 8)
            if (limited.length > 0) {
              setAtOverlay({ visible: true, selectedIndex: 0, entries: limited, query, atPos: atIndex, browsePath: null, breadcrumbs: [], pinned: false })
            } else {
              setAtOverlay(null)
            }
          } catch {
            setAtOverlay(null)
          }
        }, 60)
      } else {
        setAtOverlay(null)
      }
    } else {
      setAtOverlay(null)
    }
  }
  return (
    <div
      className="relative z-10 shrink-0 px-10 pb-2.5 pt-0 transition-[padding-right] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
      style={{ paddingRight: `${40 + contentRightInset}px` }}
    >
      <div className="relative mx-auto flex w-full flex-col transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]" style={{ maxWidth: `${contentMaxWidth}px` }}>
        {/* Slash command overlay – anchored to composer top edge */}
        {slashOverlayState?.visible && onSlashSelect && (
          <SlashCommandSuggestions
            suggestions={slashOverlayState.suggestions}
            selectedIndex={slashOverlayState.selectedIndex}
            rawInput={slashOverlayState.rawInput}
            onSelect={onSlashSelect}
          />
        )}

        {/* @-mention file overlay – anchored to composer top edge */}
        {atOverlayState?.visible && onAtSelect && (
          <AtFileSuggestions
            entries={atOverlayState.entries}
            selectedIndex={atOverlayState.selectedIndex}
            query={atOverlayState.query}
            browsePath={atOverlayState.browsePath}
            breadcrumbs={atOverlayState.breadcrumbs}
            onSelect={(entry) => onAtSelect(entry, { atPos: atOverlayState.atPos, query: atOverlayState.query })}
            onNavigateInto={onNavigateIntoFolder}
            onNavigateUp={onNavigateUpFolder}
          />
        )}

        {dropItems.length > 0 ? (
          <div className="mb-3 flex flex-wrap gap-1.5 px-1">
            {dropItems.map((item) => {
              if (item.isMention) {
                const isFolder = item.kind === 'folder'
                return (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => onRemoveDropItem(item.id)}
                    className="group inline-flex max-w-full items-center gap-1.5 rounded-lg border border-jade/20 bg-jade/[0.07] px-2.5 py-1 text-left text-jade/80 transition-all duration-200 hover:border-jade/30 hover:bg-jade/[0.12] active:scale-[0.97]"
                    title={item.path}
                  >
                    {isFolder ? (
                      /* Folder badge — slightly deeper tint + folder icon */
                      <span className="flex size-[18px] shrink-0 items-center justify-center rounded-[4px] bg-jade/[0.18] text-jade">
                        <Folder className="size-2.5" />
                      </span>
                    ) : (
                      /* File badge — @ symbol */
                      <span className="flex size-[18px] shrink-0 items-center justify-center rounded-[4px] bg-jade/[0.14] text-[10px] font-bold text-jade">
                        @
                      </span>
                    )}
                    <span className="truncate text-[12px] font-semibold tracking-tight">
                      {item.name}{isFolder ? <span className="opacity-40">/</span> : null}
                    </span>
                    <span className="flex size-[14px] shrink-0 items-center justify-center rounded-full opacity-40 transition-opacity group-hover:opacity-70">
                      <X className="h-2.5 w-2.5" />
                    </span>
                  </button>
                )
              }
              // Regular drag-drop chip: neutral
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => onRemoveDropItem(item.id)}
                  className="group inline-flex max-w-full items-center gap-2 rounded-[6px] border border-border/50 bg-surface-raised px-3 py-2 text-left text-foreground/60 shadow-xs transition-all duration-200 hover:-translate-y-0.5 hover:bg-surface"
                  title={item.path}
                >
                  <span className="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground/60">
                    {item.kind === 'file' ? <Paperclip className="h-4 w-4" /> : <Folder className="h-4 w-4" />}
                  </span>
                  <span className="truncate text-[12.5px] font-medium tracking-[-0.015em]">{item.name}</span>
                  <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full text-muted-foreground/40 transition-colors group-hover:text-muted-foreground">
                    <X className="h-3.5 w-3.5" />
                  </span>
                </button>
              )
            })}
          </div>
        ) : null}

        {/* ── Composer box ── */}
        <div
          className="flex flex-col rounded-2xl bg-card px-4 pt-2 pb-0 transition-all duration-200"
          style={{
            border: `1px solid ${isComposerFocused ? 'rgba(0,0,0,0.13)' : 'rgba(0,0,0,0.08)'}`,
            boxShadow: isComposerFocused
              ? '0 4px 6px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.09), 0 20px 48px rgba(0,0,0,0.06), 0 0 0 0.5px rgba(0,0,0,0.05)'
              : '0 2px 16px rgba(0,0,0,0.07), 0 0 0 0.5px rgba(0,0,0,0.04)',
            transform: isComposerFocused ? 'translateY(-1px)' : 'translateY(0)',
          }}
        >
          {/* Textarea (align top) */}
          <div className="flex-1 px-0 pt-1 pb-0.5">
            <Textarea
              ref={textareaRef}
              data-composer-input="true"
              value={input}
              onChange={(e) => handleInputWithSlashDetect(e.target.value)}
              onCompositionStart={() => { isComposingRef.current = true }}
              onCompositionEnd={() => {
                // Defer by one tick: the Enter that triggered compositionend fires
                // its keydown *before* this event on macOS, so delaying ensures the
                // guard is still active when that keydown is processed.
                setTimeout(() => { isComposingRef.current = false }, 0)
              }}
              onKeyDown={guardedHandleKeyDown}
              onDragOver={(event) => {
                event.preventDefault()
                event.dataTransfer.dropEffect = 'copy'
                setIsDropTarget(true)
              }}
              onDragLeave={() => setIsDropTarget(false)}
              onDrop={(event) => {
                event.preventDefault()
                const structuredPayload = event.dataTransfer.getData('application/x-if2ai-rail-entry').trim()
                if (structuredPayload) {
                  try {
                    const parsed = JSON.parse(structuredPayload) as { path: string; name: string; kind: 'file' | 'folder' }
                    if (parsed.path && parsed.name && (parsed.kind === 'file' || parsed.kind === 'folder')) {
                      onFileReferenceDrop({
                        id: `${parsed.kind}:${parsed.path}`,
                        path: parsed.path,
                        name: parsed.name,
                        kind: parsed.kind,
                      })
                    }
                  } catch (error) {
                    console.error('Failed to parse dropped rail entry:', error)
                  }
                }
                setIsDropTarget(false)
              }}
              disabled={isLoading}
              rows={1}
              onFocus={() => setIsComposerFocused(true)}
              onBlur={() => setIsComposerFocused(false)}
              className="min-h-[46px] max-h-[180px] resize-none rounded-none border-0 !bg-transparent px-0 py-0 text-[13px] leading-6 shadow-none focus-visible:ring-0 placeholder:text-muted-foreground disabled:!bg-transparent disabled:opacity-100"
              placeholder="向 AI 提问，@ 添加文件，/ 输入命令，$ 使用技能"
            />
          </div>

          {/* Bottom bar — mirrors Home composer: [+ | permission] … [model | | mic | send] */}
          {/* -mx-4 px-4 lets the border-t bleed to the card edges while keeping content aligned */}
          <div className="flex items-center justify-between border-t border-border/55 -mx-4 px-4 pt-2 pb-2">
            {/* Left: attach + permission */}
            <div className="flex items-center gap-1">
              <button
                type="button"
                className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                aria-label="添加附件"
                disabled={isLoading}
              >
                <Plus className="h-[15px] w-[15px]" />
              </button>

              {/* Permission mode */}
              <PermissionModePicker
                permissionMode={permissionMode}
                setPermissionMode={setPermissionMode}
                disabled={isLoading}
              />
            </div>

            {/* Right: model | divider | mic | send */}
            <div className="flex items-center gap-1.5">
              {/* Model selector — Popover-based, BranchPicker-style */}
              <ModelPicker
                availableItems={
                  availableModelItems.length > 0 ? availableModelItems : modelItems
                }
                selected={selectedModel}
                disabled={isLoading}
                onChange={(value) => {
                  setSelectedModel(value)
                  if (availableModelItems.length > 0) {
                    const parts = value.split('/')
                    if (parts.length === 2) {
                      void setActiveModel(parts[0], parts[1])
                    }
                  }
                }}
              />

              <div className="h-3.5 w-px bg-border/70" />

              {/* 语音输入按钮（SenseVoice STT） */}
              <React.Suspense fallback={null}>
                <SttButtonLazy
                  onTranscribe={(text) => {
                    // 把转写结果追加到现有 input 末尾
                    const next = input ? `${input.trim()} ${text}` : text
                    onInputChange(next)
                  }}
                />
              </React.Suspense>

              {/* Send / Stop */}
              <button
                type="button"
                onClick={() => {
                  if (isLoading && onStop) {
                    onStop()
                  } else if (!isLoading) {
                    onSubmit()
                  }
                }}
                disabled={!isLoading && !input.trim()}
                className={cn(
                  'flex h-7 w-7 shrink-0 items-center justify-center rounded-full transition-all duration-150 active:scale-95',
                  isLoading
                    ? 'bg-destructive text-white hover:opacity-90'
                    : input.trim()
                      ? 'bg-primary text-primary-foreground hover:opacity-90'
                      : 'bg-muted text-muted-foreground cursor-not-allowed'
                )}
                aria-label={isLoading ? '停止生成' : '发送消息'}
              >
                {isLoading ? (
                  <Square className="h-3 w-3 fill-current" />
                ) : (
                  <ArrowUp className="h-[14px] w-[14px]" />
                )}
              </button>
            </div>
          </div>

          {/* Drop target overlay */}
          {isDropTarget && (
            <div className="pointer-events-none absolute inset-x-4 top-3 rounded-xl border border-dashed border-border bg-surface/88 px-4 py-3 text-[12px] text-muted-foreground backdrop-blur-sm">
              文件会作为附件发送，文件夹会作为引用附加到消息里。
            </div>
          )}
        </div>

        {/* ── Project context bar (project | workdir | branch) ── */}
        {(projectLabel || workdirLabel) && (
          <div className="mt-1.5 flex items-center gap-2 px-1 pb-0.5">
            {projectLabel && (
              <button
                type="button"
                onClick={onProjectPillClick}
                className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
              >
                <FolderOpen className="h-[11px] w-[11px]" />
                {projectLabel}
              </button>
            )}
            {workdirLabel && (
              <span className="flex items-center gap-1 text-[11px] text-muted-foreground/80">
                <Laptop className="h-[11px] w-[11px]" />
                {workdirLabel}
              </span>
            )}
            <BranchPicker
              cwd={defaultWorkdir}
              currentBranch={branchLabel}
              onChanged={onBranchChange}
              isGitRepo={isGitRepo ?? null}
              onInitRepo={onGitRepoChanged}
            />
          </div>
        )}
      </div>
    </div>
  )
})
