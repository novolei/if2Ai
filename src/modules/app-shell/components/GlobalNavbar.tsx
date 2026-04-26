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
  const updateBannerOpen = updaterBannerVisible && updateActionActive
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
        {updateBannerOpen ? (
          <div
            className="window-no-drag absolute bottom-[72px] left-[76px] z-40 w-[478px] rounded-[24px] border border-black/[0.035] bg-[#f0f0f1]/94 p-5 text-left shadow-[0_18px_45px_rgba(15,23,42,0.14),0_1px_0_rgba(255,255,255,0.78)_inset] backdrop-blur-2xl motion-safe:animate-[if2ai-updater-banner-in_260ms_cubic-bezier(0.2,0.8,0.2,1)_both]"
            data-window-no-drag="true"
          >
            <div className="flex items-center gap-5">
              <div className="relative flex size-[78px] shrink-0 items-center justify-center">
                <div className="absolute bottom-0 h-12 w-[66px] rounded-[8px] bg-[linear-gradient(180deg,#e4b983_0%,#c28b58_100%)] shadow-[0_14px_22px_rgba(88,55,28,0.16)]" />
                <div className="absolute bottom-[34px] left-[6px] h-8 w-[34px] skew-y-[-20deg] bg-[#efc08b]" />
                <div className="absolute bottom-[34px] right-[6px] h-8 w-[34px] skew-y-[20deg] bg-[#d99a62]" />
                <div className="absolute bottom-[18px] flex flex-col items-center drop-shadow-[0_8px_10px_rgba(196,57,43,0.22)] motion-safe:animate-[if2ai-updater-bulb-float_1.9s_ease-in-out_infinite]">
                  <div className="relative h-[48px] w-[36px] rounded-[52%_52%_48%_48%] bg-[radial-gradient(circle_at_34%_24%,rgba(255,255,255,0.72)_0_10%,transparent_28%),linear-gradient(145deg,#ff6a57_0%,#f5392d_58%,#c92621_100%)] shadow-[inset_-8px_-10px_16px_rgba(112,22,17,0.18),inset_5px_5px_12px_rgba(255,255,255,0.18)]">
                    <div className="absolute left-1/2 top-[9px] h-[30px] w-[2px] -translate-x-1/2 rounded-full bg-white/30" />
                  </div>
                  <div className="-mt-[3px] h-0 w-0 border-x-[5px] border-t-[7px] border-x-transparent border-t-[#c92621]" />
                </div>
                <div className="absolute left-[12px] top-[4px] h-[18px] w-[18px]">
                  <div className="absolute left-1/2 top-0 h-full w-[3px] -translate-x-1/2 rounded-full bg-[#ffc640]" />
                  <div className="absolute left-0 top-1/2 h-[3px] w-full -translate-y-1/2 rounded-full bg-[#ffc640]" />
                  <div className="absolute left-1/2 top-1/2 h-[3px] w-full -translate-x-1/2 -translate-y-1/2 rotate-45 rounded-full bg-[#ffd86b]" />
                  <div className="absolute left-1/2 top-1/2 h-[3px] w-full -translate-x-1/2 -translate-y-1/2 -rotate-45 rounded-full bg-[#ffd86b]" />
                </div>
                <ArrowUpCircle className="absolute bottom-[9px] h-5 w-5 text-[#9b704b]/80" strokeWidth={2.25} />
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-[28px] font-semibold leading-tight tracking-[-0.045em] text-[#171717]">发现新版本!</div>
                <div className="mt-1 text-[12.5px] leading-5 text-black/42">
                  {updaterLatestVersion
                    ? `If2Ai v${updaterLatestVersion.replace(/^v/, '')} 已准备好安装`
                    : 'If2Ai 已准备好安装'}
                </div>
              </div>
              <button
                type="button"
                className="shrink-0 rounded-full bg-[#222222] px-8 py-3 text-[22px] font-bold tracking-[-0.03em] text-white shadow-[0_8px_18px_rgba(0,0,0,0.16)] transition-all hover:-translate-y-0.5 hover:bg-black active:translate-y-0"
                onClick={onRunUpdater}
              >
                更新
              </button>
              <button
                type="button"
                aria-label="关闭更新提示"
                className="absolute right-3 top-3 flex size-7 shrink-0 items-center justify-center rounded-full text-black/25 transition-colors hover:bg-black/[0.05] hover:text-black/55"
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
          iconMotion={updateActionBusy ? 'vertical-loop' : 'none'}
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
