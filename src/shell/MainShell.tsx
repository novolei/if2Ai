// Phase M2.7 — minimal main shell.
//
// Owns the chrome that wraps the post-boot main surface:
//
//   - `<AppVersionWatermark/>`   bottom-right version label.
//   - `<ExecutionModePill/>`     fixed top-right runtime-projection
//                                consumer (M2.6 visible surface).
//   - `<GlobalNavbar/>`          left rail.
//   - `<main>` background gradients + scroll container.
//
// Everything else (chat workspace, dialogs, drawers, etc.) is
// passed via `children` so App.tsx keeps owning the data flow into
// those components during the M2 transition.
//
// Honest scope (mirrors the M2.7 task):
//   - This is a **container**, not a router.  `activeSection`
//     decision logic stays in `App.tsx`; the shell only forwards
//     the section + section-handler props into `<GlobalNavbar/>`.
//   - The shell mounts `ExecutionModePill` directly because the
//     pill is a self-gating projection consumer — it renders
//     nothing until `snapshot.executionMode` exists.  The shell
//     does NOT recompute the decision.
//   - There is no specialized-surface routing.  When that lands
//     it should consume the same projection store (`snapshot
//     .executionMode.routeHint`) and either short-circuit
//     `children` or replace this shell with a
//     specialized-surface variant.

import type { ReactNode } from 'react'

import { AppVersionWatermark } from '@/components/AppVersionWatermark'
import { ExecutionModePill } from '@/modules/execution-mode/ExecutionModePill'
import { GlobalNavbar } from '@/modules/app-shell/components/GlobalNavbar'
import type { AppSection } from '@/modules/app-shell/types'

export interface MainShellNavbarProps {
  activeSection: AppSection
  onSelectSection: (section: AppSection) => void
  onOpenSettings: () => void
  onStartWindowDrag: (event: { clientX: number; clientY: number }) => void
  appIconSrc: string
}

export interface MainShellProps {
  /** Props forwarded into `<GlobalNavbar/>`. */
  navbar: MainShellNavbarProps
  /** Main-surface body (chat workspace / memory browser / section
   * workspace) plus any session-scoped dialogs / drawers. */
  children: ReactNode
}

export function MainShell({ navbar, children }: MainShellProps) {
  return (
    <div
      className="relative isolate grid h-screen min-h-0 min-w-0 overflow-hidden bg-[#f6f7f8] text-foreground"
      style={{ gridTemplateColumns: '76px minmax(0, 1fr)' }}
    >
      <AppVersionWatermark />
      {/* Phase M2.6 — minimal execution-mode visible surface.
          Self-gating projection consumer; renders nothing until
          `snapshot.executionMode != null`. */}
      <div className="pointer-events-none fixed right-4 top-3 z-40">
        <ExecutionModePill className="pointer-events-auto" />
      </div>
      <GlobalNavbar
        activeSection={navbar.activeSection}
        onSelectSection={navbar.onSelectSection}
        onOpenSettings={navbar.onOpenSettings}
        onStartWindowDrag={navbar.onStartWindowDrag}
        appIconSrc={navbar.appIconSrc}
      />
      <main className="relative z-10 flex min-h-0 min-w-0 flex-col overflow-hidden bg-[#f6f7f8]">
        <div aria-hidden="true" className="pointer-events-none absolute inset-0 z-0">
          <div className="absolute inset-0 bg-[#f6f7f8]" />
          <div className="absolute inset-0 bg-[linear-gradient(135deg,rgba(246,247,248,0)_0%,rgba(246,247,248,0.12)_46%,rgba(246,247,248,0.76)_100%)]" />
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_8%,rgba(255,255,255,0.96)_0%,rgba(255,255,255,0.76)_18%,rgba(255,255,255,0)_52%),radial-gradient(circle_at_50%_100%,rgba(242,244,246,0.94)_0%,rgba(242,244,246,0.62)_34%,rgba(242,244,246,0.18)_68%,rgba(242,244,246,0)_100%)]" />
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_72%_78%,rgba(255,255,255,0.5),transparent_28%),radial-gradient(circle_at_86%_90%,rgba(242,244,246,0.34),transparent_30%)]" />
        </div>
        <div className="relative z-10 flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          {children}
        </div>
      </main>
    </div>
  )
}
