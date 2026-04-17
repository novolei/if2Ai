import type { MouseEvent as ReactMouseEvent } from 'react'
import { ArrowUpCircle, Brain, MessageSquare, Settings } from 'lucide-react'
import type { AppSection } from '../types'
import { NavTooltipButton } from './NavTooltipButton'

export function GlobalNavbar({
  activeSection,
  onSelectSection,
  onOpenSettings,
  onStartWindowDrag,
  appIconSrc,
}: {
  activeSection: AppSection
  onSelectSection: (section: AppSection) => void
  onOpenSettings: () => void
  onStartWindowDrag: (event: ReactMouseEvent<HTMLElement>) => void
  appIconSrc: string
}) {
  return (
    <aside
      className="relative z-30 flex h-full min-h-0 w-[76px] flex-col items-center border-r border-black/5 bg-[#f2f3f4]/56 pt-14 pb-4 select-none backdrop-blur-[2px]"
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
        <NavTooltipButton
          icon={ArrowUpCircle}
          label="检查更新"
          ghost
          onClick={() => {}}
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
