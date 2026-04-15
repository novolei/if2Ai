import * as React from 'react'
import {
  ArrowUpRight,
  Check,
  ChevronRight,
  LoaderCircle,
  Quote,
  Save,
  ScanSearch,
  X,
} from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { cn } from '@/lib/utils'
import { ArtifactEditor } from '@/components/ui/ArtifactEditor'
import type { FilePreviewPayload } from '@/lib/tauri'

type SaveState = 'idle' | 'saving' | 'saved' | 'error'

export interface ProjectPreviewPanelProps {
  tabs: FilePreviewPayload[]
  activeTabPath: string | null
  drafts: Record<string, string>
  saveStates: Record<string, SaveState>
  dirtyPaths: string[]
  onSelectTab: (path: string) => void
  onCloseTab: (path: string) => void
  onClosePanel: () => void
  onOpenExternally: (preview: FilePreviewPayload) => void
  onQuoteIntoChat: (preview: FilePreviewPayload) => void
  onInsertIntoChat: (preview: FilePreviewPayload) => void
  onChangeDraft: (path: string, value: string) => void
  onSaveNow: (path: string) => void
}

export function ProjectPreviewPanel({
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
}: ProjectPreviewPanelProps) {
  const [viewModes, setViewModes] = React.useState<Record<string, 'preview' | 'edit'>>({})
  const activePreview = tabs.find((tab) => tab.path === activeTabPath) ?? tabs[0] ?? null
  const activeDraft = activePreview ? drafts[activePreview.path] ?? activePreview.content ?? '' : ''
  const saveState = activePreview ? saveStates[activePreview.path] ?? 'idle' : 'idle'
  const isDirty = activePreview ? dirtyPaths.includes(activePreview.path) : false
  const activeViewMode = activePreview ? viewModes[activePreview.path] ?? (activePreview.kind === 'html' ? 'preview' : 'edit') : 'preview'
  const dataUrl = React.useMemo(() => {
    if (!activePreview?.data_base64 || !activePreview.mime_type) return null
    return `data:${activePreview.mime_type};base64,${activePreview.data_base64}`
  }, [activePreview])

  return (
    <aside className="relative flex h-full min-h-0 min-w-0 flex-col border-l border-border/50 bg-inspector">
      <div className="flex items-center gap-1 px-3 pb-1 pt-3">
        <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
          {tabs.map((tab) => {
            const active = tab.path === activePreview?.path
            const dirty = dirtyPaths.includes(tab.path)
            return (
              <button
                key={tab.path}
                type="button"
                onClick={() => onSelectTab(tab.path)}
                className={cn(
                  'group inline-flex max-w-[180px] shrink-0 items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-[11.5px] transition-all duration-150',
                  active
                    ? 'border-border bg-surface-raised text-foreground shadow-xs'
                    : 'border-transparent bg-transparent text-muted-foreground hover:bg-muted'
                )}
              >
                <span className="truncate">{tab.name}</span>
                {dirty ? <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-destructive/60" /> : null}
                <span
                  className="inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-[6px] text-muted-foreground opacity-50 transition-all hover:bg-muted hover:text-foreground/70 hover:opacity-100"
                  onClick={(event) => {
                    event.stopPropagation()
                    onCloseTab(tab.path)
                  }}
                >
                  <X className="h-3 w-3" />
                </span>
              </button>
            )
          })}
        </div>
        <button
          type="button"
          title="关闭预览"
          onClick={onClosePanel}
          className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-lg text-muted-foreground/60 transition-colors hover:bg-muted hover:text-foreground/70"
        >
          <ChevronRight className="h-4 w-4" />
        </button>
      </div>

      {activePreview ? (
        <>
          <div className="flex items-center justify-between gap-3 px-4 pb-2 pt-1">
            <div className="min-w-0">
              <div className="truncate text-[14px] font-medium tracking-[-0.02em] text-foreground">{activePreview.name}</div>
              <div className="truncate pt-0.5 text-[11px] text-muted-foreground/60">{activePreview.path}</div>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
              {activePreview.editable && (activePreview.kind === 'html' || activePreview.kind === 'markdown') ? (
                <div className="inline-flex items-center rounded-lg border border-border bg-surface/94 p-0.5">
                  <button
                    type="button"
                    onClick={() => setViewModes((current) => ({ ...current, [activePreview.path]: 'preview' }))}
                    className={cn(
                      'rounded-lg px-2.5 py-1 text-[11px] transition-colors',
                      activeViewMode === 'preview' ? 'bg-accent text-foreground' : 'text-muted-foreground'
                    )}
                  >
                    预览
                  </button>
                  <button
                    type="button"
                    onClick={() => setViewModes((current) => ({ ...current, [activePreview.path]: 'edit' }))}
                    className={cn(
                      'rounded-lg px-2.5 py-1 text-[11px] transition-colors',
                      activeViewMode === 'edit' ? 'bg-accent text-foreground' : 'text-muted-foreground'
                    )}
                  >
                    编辑
                  </button>
                </div>
              ) : null}
              <PreviewActionButton onClick={() => onOpenExternally(activePreview)} icon={<ArrowUpRight className="h-3.5 w-3.5" />}>
                外部打开
              </PreviewActionButton>
              <PreviewActionButton onClick={() => onQuoteIntoChat(activePreview)} icon={<Quote className="h-3.5 w-3.5" />}>
                引用
              </PreviewActionButton>
              <PreviewActionButton onClick={() => onInsertIntoChat(activePreview)} icon={<ScanSearch className="h-3.5 w-3.5" />}>
                上下文
              </PreviewActionButton>
              {activePreview.editable ? (
                <button
                  type="button"
                  onClick={() => onSaveNow(activePreview.path)}
                  className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-border bg-surface px-3 text-[12px] text-muted-foreground shadow-xs transition-colors hover:bg-surface-raised"
                >
                  {saveState === 'saving' ? <LoaderCircle className="h-3.5 w-3.5 animate-spin" /> : saveState === 'saved' ? <Check className="h-3.5 w-3.5" /> : <Save className="h-3.5 w-3.5" />}
                  <span>{saveState === 'saving' ? '保存中' : saveState === 'saved' ? '已保存' : isDirty ? '保存' : '手动保存'}</span>
                </button>
              ) : null}
            </div>
          </div>

          <div className="min-h-0 flex-1 px-3 pb-3">
            <div className="h-full min-h-0 overflow-hidden rounded-[22px] border border-border bg-surface-raised shadow-xs">
              {activePreview.kind === 'image' && dataUrl ? (
                <div className="flex h-full items-center justify-center p-5">
                  <img src={dataUrl} alt={activePreview.name} className="preview-image max-h-full max-w-full object-contain" />
                </div>
              ) : activePreview.kind === 'video' && dataUrl ? (
                <div className="flex h-full items-center justify-center bg-muted p-5">
                  <video src={dataUrl} controls className="max-h-full max-w-full rounded-[18px] bg-black shadow-lg" />
                </div>
              ) : activePreview.kind === 'pdf' && dataUrl ? (
                <iframe title={activePreview.name} src={dataUrl} className="h-full w-full border-0" />
              ) : activePreview.kind === 'html' ? (
                activeViewMode === 'edit' && activePreview.editable ? (
                  <ArtifactEditor content={activeDraft} language={activePreview.language} onChange={(value) => onChangeDraft(activePreview.path, value)} />
                ) : (
                  <iframe title={activePreview.name} srcDoc={activeDraft || activePreview.content || ''} className="h-full w-full border-0 bg-white" sandbox="allow-scripts allow-same-origin" />
                )
              ) : activePreview.kind === 'markdown' ? (
                activeViewMode === 'edit' && activePreview.editable ? (
                  <ArtifactEditor content={activeDraft} language={activePreview.language} onChange={(value) => onChangeDraft(activePreview.path, value)} />
                ) : (
                  <div className="h-full overflow-auto px-6 py-5 text-[13px] leading-7 text-foreground/72 [font-family:'Iowan_Old_Style','Baskerville',ui-serif,Georgia,serif]">
                    <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                      {activeDraft || activePreview.content || ''}
                    </ReactMarkdown>
                  </div>
                )
              ) : activePreview.editable ? (
                <ArtifactEditor content={activeDraft} language={activePreview.language} onChange={(value) => onChangeDraft(activePreview.path, value)} />
              ) : (
                <div className="h-full overflow-auto px-5 py-5">
                  <pre className="preview-code whitespace-pre-wrap break-words text-[12px] leading-6 text-foreground/72">
                    {activePreview.content ?? ''}
                  </pre>
                </div>
              )}
            </div>
          </div>
        </>
      ) : (
        <div className="flex flex-1 items-center justify-center text-[13px] text-muted-foreground/60">还没有打开文件预览</div>
      )}
    </aside>
  )
}

function PreviewActionButton({
  children,
  icon,
  onClick,
}: {
  children: React.ReactNode
  icon: React.ReactNode
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-border bg-surface/94 px-3 text-[12px] text-muted-foreground transition-all duration-150 hover:-translate-y-0.5 hover:bg-surface"
    >
      {icon}
      <span>{children}</span>
    </button>
  )
}
