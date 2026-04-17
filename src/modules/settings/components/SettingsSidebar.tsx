import { Sparkles, X } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { SETTINGS_SECTIONS } from '../data'
import type { SettingsSectionId } from '../types'
import { SettingsSidebarItem } from './SettingsSidebarItem'

interface SettingsSidebarProps {
  activeSection: SettingsSectionId
  onSectionChange: (section: SettingsSectionId) => void
  onClose: () => void
}

/**
 * Start a native window drag.
 * e.preventDefault() must be called synchronously before the async handoff to
 * prevent the browser from entering text-selection mode (I-beam cursor) during
 * the brief window between mousedown and the OS taking over the drag.
 */
async function startDrag(e: React.MouseEvent<HTMLElement>) {
  if (e.button !== 0) return
  const target = e.target as HTMLElement | null
  if (target?.closest('[data-window-no-drag="true"]')) return
  e.preventDefault()
  try {
    await getCurrentWindow().startDragging()
  } catch {
    // Some sandboxed/testing contexts don't support startDragging.
  }
}

export function SettingsSidebar({ activeSection, onSectionChange, onClose }: SettingsSidebarProps) {
  return (
    <aside className="flex w-[200px] shrink-0 flex-col border-r border-black/[0.07] bg-[#f0f1f2] px-3 pb-3 select-none">

      {/* ── Traffic-light clearance + drag strip ──────────────────────────
          macOS overlays the close/minimize/zoom buttons (~12-24 px from top)
          in the top-left corner. This 36-px invisible strip keeps them clear
          and also acts as a drag zone. */}
      <div
        className="h-9 w-full shrink-0 cursor-default"
        onMouseDown={(e) => void startDrag(e)}
        aria-hidden
      />

      {/* ── Logo + close ── */}
      <div
        className="mb-4 flex cursor-default items-center justify-between px-1"
        onMouseDown={(e) => void startDrag(e)}
      >
        {/* draggable={false} prevents the browser from treating the icon as a
            draggable image element, which would also show an I-beam cursor */}
        <div className="flex items-center gap-2 pointer-events-none">
          <div
            className="flex size-6 items-center justify-center rounded-[7px] bg-jade"
            style={{ boxShadow: '0 1.5px 4px color-mix(in oklch, var(--jade) 40%, transparent)' }}
          >
            <Sparkles className="size-3 text-white" strokeWidth={1.8} />
          </div>
          <div>
            <div className="text-[12.5px] font-bold tracking-tight text-foreground/80">If2Ai</div>
            <div className="text-[9.5px] font-semibold uppercase tracking-[0.12em] text-black/25">
              设置
            </div>
          </div>
        </div>

        <button
          type="button"
          data-window-no-drag="true"
          onClick={onClose}
          className="pointer-events-auto flex size-6 items-center justify-center rounded-lg text-black/30 transition-colors hover:bg-black/[0.07] hover:text-black/60"
          aria-label="关闭设置"
        >
          <X className="size-3.5" />
        </button>
      </div>

      {/* ── Nav ── */}
      <nav className="flex flex-1 flex-col gap-0.5">
        {SETTINGS_SECTIONS.map((section) => (
          <SettingsSidebarItem
            key={section.id}
            icon={section.icon}
            label={section.label}
            active={section.id === activeSection}
            onClick={() => onSectionChange(section.id)}
          />
        ))}
      </nav>
    </aside>
  )
}
