import type { MouseEvent as ReactMouseEvent } from 'react'
import { ArrowUpCircle, Brain, MessageSquare, Settings, Swords, X } from 'lucide-react'
import { toast } from 'sonner'
import { useJiaochangI18n } from '@/modules/jiaochang/i18n/useJiaochangI18n'
import type { AppSection } from '../types'
import { NavTooltipButton } from './NavTooltipButton'

export function GlobalNavbar({
  activeSection,
  onSelectSection,
  onOpenSettings,
  onRunUpdater,
  updaterStatus = 'idle',
  updaterLatestVersion,
  updaterBannerVisible = false,
  onDismissUpdaterBanner,
  onStartWindowDrag,
  appIconSrc,
}: {
  activeSection: AppSection
  onSelectSection: (section: AppSection) => void
  onOpenSettings: () => void
  onRunUpdater?: () => void
  updaterStatus?: 'idle' | 'available' | 'checking' | 'downloading' | 'downloaded' | 'installing' | 'latest' | 'error'
  updaterLatestVersion?: string | null
  updaterBannerVisible?: boolean
  onDismissUpdaterBanner?: () => void
  onStartWindowDrag: (event: ReactMouseEvent<HTMLElement>) => void
  appIconSrc: string
}) {
  const { t: tJiaochang } = useJiaochangI18n()
  const updateActionActive = updaterStatus === 'available'
  const updateActionBusy = updaterStatus === 'checking' || updaterStatus === 'downloading' || updaterStatus === 'downloaded' || updaterStatus === 'installing'
  const updateLabel = updateActionActive
    ? `下载更新${updaterLatestVersion ? ` v${updaterLatestVersion.replace(/^v/, '')}` : ''}`
    : updateActionBusy
      ? '正在处理更新'
      : '检查更新'

  return (
    <aside
      className="relative z-30 flex h-full min-h-0 w-[76px] flex-col items-center border-r border-border/50 bg-[var(--app-rail-bg,rgba(242,243,244,0.56))] pt-14 pb-4 select-none backdrop-blur-[2px]"
      onMouseDown={onStartWindowDrag}
    >
      <div className="window-no-drag flex flex-col items-center gap-5" data-window-no-drag="true">
        {/* Logo container — multi-layer shadow for depth + inset highlight */}
        <div
          className="relative size-12 overflow-hidden rounded-[13px]"
          style={{
            boxShadow: [
              '0 1px 0 0.5px rgba(255,255,255,0.55)',   /* top edge highlight — glass rim */
              '0 0 0 0.5px rgba(0,0,0,0.12)',            /* hairline border */
              '0 2px 4px rgba(0,0,0,0.18)',              /* contact shadow */
              '0 6px 16px rgba(0,0,0,0.18)',             /* mid diffuse */
              '0 14px 28px rgba(0,0,0,0.12)',            /* long ambient */
            ].join(','),
          }}
        >
          <img
            src={appIconSrc}
            alt="If2Ai"
            className="size-full object-cover"
            draggable={false}
          />
          {/* Inset gloss — top-left to center, simulates convex surface */}
          <div
            className="pointer-events-none absolute inset-0 rounded-[13px]"
            style={{
              background:
                'linear-gradient(145deg, rgba(255,255,255,0.18) 0%, rgba(255,255,255,0.06) 38%, transparent 60%)',
            }}
          />
        </div>
        <div className="flex flex-col gap-3.5">
          <NavTooltipButton
            icon={MessageSquare}
            label="Chat"
            active={activeSection === 'chat'}
            onClick={() => onSelectSection('chat')}
          />
          <NavTooltipButton
            icon={Brain}
            label="记忆"
            active={activeSection === 'memory'}
            onClick={() => onSelectSection('memory')}
          />
          <NavTooltipButton
            icon={Swords}
            label={tJiaochang('app.title')}
            active={activeSection === 'jiaochang'}
            onClick={() => onSelectSection('jiaochang')}
          />
          {/* TODO(F6): 连接 Skill 系统后启用 Skills 图标 */}
          {/* <NavTooltipButton
            icon={Sparkles}
            label="技能和应用"
            active={activeSection === 'skills'}
            onClick={() => onSelectSection('skills')}
          /> */}
          {/* TODO: 连接自动化系统后启用 Automation 图标 */}
          {/* <NavTooltipButton
            icon={Clock3}
            label="自动化"
            active={activeSection === 'automation'}
            onClick={() => onSelectSection('automation')}
          /> */}
        </div>
      </div>

      <div className="mt-auto flex flex-col items-center gap-2 pb-1">
        {updaterBannerVisible ? (
          <div
            className="window-no-drag absolute bottom-[70px] left-[66px] z-40 w-[258px] rounded-xl border border-primary/20 bg-popover/92 p-3 text-left shadow-[0_14px_40px_rgba(18,45,34,0.16),0_1px_0_rgba(255,255,255,0.45)_inset] backdrop-blur-xl"
            data-window-no-drag="true"
          >
            <div className="flex items-start gap-2.5">
              <div className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-lg bg-jade/10 text-jade">
                <ArrowUpCircle className="h-4 w-4" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-semibold text-foreground/88">发现新版本</div>
                <div className="mt-0.5 text-[12px] leading-5 text-muted-foreground">
                  {updaterLatestVersion
                    ? `If2Ai v${updaterLatestVersion.replace(/^v/, '')} 已准备好下载。`
                    : 'If2Ai 已准备好下载新版本。'}
                </div>
                <button
                  type="button"
                  className="mt-2 rounded-md bg-jade px-2.5 py-1 text-[12px] font-semibold text-white shadow-sm transition-colors hover:bg-jade-dim"
                  onClick={onRunUpdater}
                >
                  下载并安装
                </button>
              </div>
              <button
                type="button"
                aria-label="关闭更新提示"
                className="flex size-6 shrink-0 items-center justify-center rounded-md text-muted-foreground/70 transition-colors hover:bg-accent hover:text-accent-foreground"
                onClick={onDismissUpdaterBanner}
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          </div>
        ) : null}
        <NavTooltipButton
          icon={ArrowUpCircle}
          label={updateLabel}
          ghost
          variant={updateActionActive || updateActionBusy ? 'update' : 'default'}
          iconClassName={updateActionActive ? 'motion-safe:animate-[if2ai-updater-arrow_1.25s_ease-in-out_infinite]' : undefined}
          disabled={updateActionBusy}
          onClick={onRunUpdater ?? (() =>
            toast.info('自动更新暂未接入', {
              description: '当前版本可在「设置 > 关于」查看；后续会在这里显示检查进度。',
            })
          )}
        />
        <NavTooltipButton
          icon={Settings}
          label="设置"
          ghost
          onClick={onOpenSettings}
        />
      </div>
    </aside>
  )
}
