import * as React from 'react'
import {
  ArrowUpRight,
  Check,
  ChevronRight,
  GitCompare,
  LoaderCircle,
  Maximize2,
  Quote,
  RefreshCw,
  Save,
  ScanSearch,
  X,
} from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { cn } from '@/lib/utils'
import { ArtifactEditor } from '@/components/ui/ArtifactEditor'
import { WriteToolDiffCard } from '@/components/chat/WriteToolDiffCard'
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
  /** 强制从磁盘重新读取该 tab 的内容并替换原文。
   *  外部编辑器（VS Code / vim）改了文件、或 agent 流尚未走完
   *  file_write 自动刷新时的兜底入口。 */
  onRefresh?: (path: string) => void | Promise<void>
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
  onRefresh,
}: ProjectPreviewPanelProps) {
  // 'diff' = 草稿 vs 磁盘对比；'preview' = 渲染（markdown 文档 / HTML
  // 浏览器视图）；'edit' = 源代码 / 文本编辑器。后两个状态对纯 code
  // 文件没意义（永远 edit）；diff 仅在 dirty 时可用。
  const [viewModes, setViewModes] = React.useState<Record<string, 'preview' | 'edit' | 'diff'>>({})
  const [refreshing, setRefreshing] = React.useState<string | null>(null)
  const [imageZoomed, setImageZoomed] = React.useState<Record<string, boolean>>({})
  const activePreview = tabs.find((tab) => tab.path === activeTabPath) ?? tabs[0] ?? null
  const activeDraft = activePreview ? drafts[activePreview.path] ?? activePreview.content ?? '' : ''
  const saveState = activePreview ? saveStates[activePreview.path] ?? 'idle' : 'idle'
  const isDirty = activePreview ? dirtyPaths.includes(activePreview.path) : false
  const rawViewMode = activePreview ? viewModes[activePreview.path] : undefined
  // 用户没主动选过模式时的默认：HTML → 浏览器预览；其它 → 编辑（代
  // 码视图）。如果用户停在 'diff' 但当前已经没改动了，回退到 edit
  // 避免一个空 diff 卡片占着面板。
  const activeViewMode: 'preview' | 'edit' | 'diff' = activePreview
    ? rawViewMode === 'diff' && !isDirty
      ? 'edit'
      : rawViewMode ?? (activePreview.kind === 'html' ? 'preview' : 'edit')
    : 'preview'
  const dataUrl = React.useMemo(() => {
    if (!activePreview?.data_base64 || !activePreview.mime_type) return null
    return `data:${activePreview.mime_type};base64,${activePreview.data_base64}`
  }, [activePreview])
  const isImageZoomed = activePreview ? imageZoomed[activePreview.path] ?? false : false
  // text 类型才有 编辑/预览/diff 模式可切；编辑能力跟着 backend 给的
  // editable 走（image/video 当然 editable=false）。
  const isTextEditable =
    activePreview?.editable &&
    (activePreview.kind === 'markdown' || activePreview.kind === 'code' || activePreview.kind === 'html')
  const supportsRendered =
    activePreview?.kind === 'markdown' || activePreview?.kind === 'html'

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
              {isTextEditable ? (
                <div className="inline-flex items-center rounded-lg border border-border bg-surface/94 p-0.5">
                  {supportsRendered && (
                    <ModeButton
                      active={activeViewMode === 'preview'}
                      onClick={() =>
                        setViewModes((c) => ({ ...c, [activePreview.path]: 'preview' }))
                      }
                    >
                      {activePreview.kind === 'html' ? '浏览器' : '预览'}
                    </ModeButton>
                  )}
                  <ModeButton
                    active={activeViewMode === 'edit'}
                    onClick={() => setViewModes((c) => ({ ...c, [activePreview.path]: 'edit' }))}
                  >
                    代码
                  </ModeButton>
                  {/* Diff 模式：仅 dirty 时启用，避免空 diff 浪费用户一次点击 */}
                  <button
                    type="button"
                    disabled={!isDirty}
                    onClick={() => setViewModes((c) => ({ ...c, [activePreview.path]: 'diff' }))}
                    className={cn(
                      'inline-flex items-center gap-1 rounded-lg px-2.5 py-1 text-[11px] outline-none transition-colors disabled:cursor-not-allowed disabled:opacity-40',
                      activeViewMode === 'diff'
                        ? 'bg-accent text-foreground'
                        : 'text-muted-foreground hover:text-foreground/70',
                    )}
                    title={isDirty ? '查看未保存改动 vs 磁盘版本' : '当前没有未保存改动'}
                  >
                    <GitCompare className="h-3 w-3" />
                    Diff
                  </button>
                </div>
              ) : null}
              {onRefresh ? (
                <PreviewActionButton
                  onClick={async () => {
                    if (refreshing === activePreview.path) return
                    setRefreshing(activePreview.path)
                    try {
                      await onRefresh(activePreview.path)
                    } finally {
                      setRefreshing(null)
                    }
                  }}
                  icon={
                    <RefreshCw
                      className={cn(
                        'h-3.5 w-3.5',
                        refreshing === activePreview.path && 'animate-spin',
                      )}
                    />
                  }
                >
                  刷新
                </PreviewActionButton>
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
              {/* Diff 模式：跨所有文本类型共用，渲染 disk → draft 的对
                  比卡片。点 mode 切回 edit/preview 即可退出。 */}
              {isTextEditable && activeViewMode === 'diff' ? (
                <div className="h-full overflow-auto p-3">
                  <WriteToolDiffCard
                    path={activePreview.name}
                    oldContent={activePreview.content ?? ''}
                    newContent={activeDraft}
                  />
                </div>
              ) : activePreview.kind === 'image' && dataUrl ? (
                <ImagePreview
                  src={dataUrl}
                  name={activePreview.name}
                  zoomed={isImageZoomed}
                  onToggleZoom={() =>
                    setImageZoomed((c) => ({
                      ...c,
                      [activePreview.path]: !isImageZoomed,
                    }))
                  }
                />
              ) : activePreview.kind === 'video' && dataUrl ? (
                // macOS 风格视频框：外层 wrapper 负责"圆角 + 阴影 +
                // 信箱区填色"，video 自身只管尺寸 + object-contain。
                //   - `overflow-hidden` 把 video 元素 paint 严格裁到
                //     圆角内，避免 WebKit 在某些版本里圆角外漏出原色
                //   - `bg-black` 给 letterbox/pillarbox 区域一个统一
                //     的电影黑（macOS QuickTime 同款），不会再像之前
                //     bg-black 直接挂在 video 上时圆角缝里显灰
                //   - `shadow-[0_8px_24px_...]` 接近 macOS Finder
                //     Quick Look 的悬浮投影
                //   - inline-block + max-w-full 让 wrapper 自适应到
                //     video 的实际显示尺寸，圆角恰好沿视频边缘走，不
                //     会把整张面板都框成圆角
                <div className="flex h-full items-center justify-center p-4">
                  <div className="inline-flex max-h-full max-w-full overflow-hidden rounded-[14px] bg-black shadow-[0_8px_24px_rgba(0,0,0,0.18),0_2px_6px_rgba(0,0,0,0.10)] ring-1 ring-black/10">
                    <video
                      src={dataUrl}
                      controls
                      autoPlay={false}
                      playsInline
                      className="block max-h-[calc(100vh-220px)] max-w-full"
                    />
                  </div>
                </div>
              ) : activePreview.kind === 'pdf' && dataUrl ? (
                <iframe title={activePreview.name} src={dataUrl} className="h-full w-full border-0" />
              ) : activePreview.kind === 'html' ? (
                activeViewMode === 'edit' && activePreview.editable ? (
                  <ArtifactEditor content={activeDraft} language={activePreview.language} onChange={(value) => onChangeDraft(activePreview.path, value)} />
                ) : (
                  // 浏览器预览：iframe + sandbox，比 BrowserCard 那套真实
                  // CDP 浏览器轻量太多（启动 ~100ms vs 几百 MB），对静
                  // 态 HTML 文件足够。`allow-scripts allow-same-origin`
                  // 让脚本能跑、同源 fetch 能拿到结果，但 sandbox 隔离
                  // 主窗口 cookies / storage。
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

/** 模式切换按钮（代码 / 预览 / Diff 三件套共用样式） */
function ModeButton({
  active,
  children,
  onClick,
}: {
  active: boolean
  children: React.ReactNode
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'rounded-lg px-2.5 py-1 text-[11px] outline-none transition-colors',
        active ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground/70',
      )}
    >
      {children}
    </button>
  )
}

/**
 * 图片预览：
 *   - 默认 `object-contain` 适配面板
 *   - 角落浮一个 maximize 按钮，点击进 zoom 模式 → 图片以原始像素显示
 *     + 容器 overflow scroll，方便看大图细节
 *   - 再点退出 zoom
 */
function ImagePreview({
  src,
  name,
  zoomed,
  onToggleZoom,
}: {
  src: string
  name: string
  zoomed: boolean
  onToggleZoom: () => void
}) {
  // macOS 风格图片预览：
  //   - 外层 surface 是面板自带的卡片底，不再做棋盘格全屏铺满
  //   - 适配模式：用 wrapper 包 img，圆角 + 阴影 + ring，棋盘格只
  //     出现在图片自身的盒子内（透明 PNG 才能看出 alpha 通道，且不
  //     会让整个面板看着像下载占位图）
  //   - 原始模式：让图片以 1:1 像素显示，wrapper 滚动；圆角不重要
  //     了（用户在缩放看细节，圆角反而干扰边缘判断），所以这时只保
  //     留棋盘背景
  //   - 右上角浮 "适配/原始" 切换按钮，跟视频预览的 macOS Quick
  //     Look 风格保持一致
  const checkerBg =
    'bg-[linear-gradient(45deg,#eaeaea_25%,transparent_25%),linear-gradient(-45deg,#eaeaea_25%,transparent_25%),linear-gradient(45deg,transparent_75%,#eaeaea_75%),linear-gradient(-45deg,transparent_75%,#eaeaea_75%)] [background-position:0_0,0_8px,8px_-8px,-8px_0] [background-size:16px_16px]'
  return (
    <div className="relative h-full w-full">
      {zoomed ? (
        // 1:1 模式：wrapper 直接 overflow-auto，棋盘铺满给透明像素背
        // 景；不强加圆角，避免大图像素被圆角剪切误导
        <div className={cn('h-full w-full overflow-auto', checkerBg)}>
          <img
            src={src}
            alt={name}
            className="preview-image block max-w-none"
            draggable={false}
          />
        </div>
      ) : (
        // 适配模式：居中放一个 macOS 风格的圆角卡，棋盘 + 阴影 +
        // ring 都挂在 wrapper 上，img 只负责 fit
        <div className="flex h-full w-full items-center justify-center p-5">
          <div
            className={cn(
              'inline-flex max-h-full max-w-full overflow-hidden rounded-[14px] shadow-[0_8px_24px_rgba(0,0,0,0.18),0_2px_6px_rgba(0,0,0,0.10)] ring-1 ring-black/10',
              checkerBg,
            )}
          >
            <img
              src={src}
              alt={name}
              className="preview-image block max-h-[calc(100vh-220px)] max-w-full object-contain"
              draggable={false}
            />
          </div>
        </div>
      )}
      <button
        type="button"
        onClick={onToggleZoom}
        className="absolute right-3 top-3 inline-flex h-8 items-center gap-1 rounded-lg border border-border bg-surface/94 px-2.5 text-[11.5px] text-muted-foreground shadow-xs outline-none transition-colors hover:bg-surface hover:text-foreground/85"
        title={zoomed ? '适配面板' : '查看原始尺寸'}
      >
        <Maximize2 className="h-3.5 w-3.5" />
        {zoomed ? '适配' : '原始'}
      </button>
    </div>
  )
}
