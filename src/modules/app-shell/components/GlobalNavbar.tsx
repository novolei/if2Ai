import type { MouseEvent as ReactMouseEvent } from 'react'
import { ArrowUpCircle, MessageSquare, Settings } from 'lucide-react'
import { Button } from '@/components/ui/button'
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
      className="relative z-30 flex h-full min-h-0 w-[88px] flex-col items-center border-r border-black/5 bg-[#f2f3f4]/56 pt-14 pb-4 select-none backdrop-blur-[2px]"
      onMouseDown={onStartWindowDrag}
    >
      <div className="window-no-drag flex flex-col items-center gap-5" data-window-no-drag="true">
        <img
          src={appIconSrc}
          alt="If2Ai"
          className="size-12 rounded-[16px] border border-black/5 bg-black object-cover shadow-sm"
        />
        <div className="flex flex-col gap-3.5">
          <NavTooltipButton
            icon={MessageSquare}
            label="Chat"
            active={activeSection === 'chat'}
            onClick={() => onSelectSection('chat')}
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
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="window-no-drag size-11 rounded-[14px] text-black/48 hover:bg-black/[0.03] hover:text-black/74"
          data-window-no-drag="true"
          aria-label="更新"
        >
          <ArrowUpCircle className="h-5 w-5" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          onClick={onOpenSettings}
          className="window-no-drag size-11 rounded-2xl text-black/52 hover:bg-black/[0.03] hover:text-black/80"
          data-window-no-drag="true"
          aria-label="设置"
        >
          <Settings className="h-5 w-5" />
        </Button>
      </div>
    </aside>
  )
}
