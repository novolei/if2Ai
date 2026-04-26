// MIG-013 — AppShell.
//
// Canonical top-level container. Owns the BootShell + MainShell
// wiring, reads the bootstrap store for the boot phase /
// startup error, and hands off the actual section content to
// [`ContentRouter`].
//
// What AppShell deliberately does NOT do:
//
// - It does NOT run the boot sequence itself — callers fire
//   [`runBootSequence`] against the bootstrap store in a
//   `useEffect`. Keeps AppShell a pure render function and
//   matches the MIG-010 / MIG-012 pattern of "boundary reads
//   store, caller writes store".
// - It does NOT own chat / session / settings state. Those
//   stay with App.tsx in this pack; MIG-014 lifts them into
//   dedicated stores.
// - It does NOT compose specialized-surface routing. When that
//   lands it consumes the same bootstrap / runtime-projection
//   stores and wraps the current return in an additional
//   branch.

import type { ReactNode } from 'react'

import { BootShell } from '@/boot/BootShell'
import { bootRouteToSurface, useBootRoute } from '@/boot/use-boot-route'
import { MainShell } from '@/shell/MainShell'
import type { AppSection } from '@/modules/app-shell/types'
import { useBootstrapState } from '@/state'

import { ContentRouter, type ContentRouterProps } from './ContentRouter.tsx'

export interface AppShellNavbarProps {
  activeSection: AppSection
  onSelectSection: (section: AppSection) => void
  onOpenSettings: () => void
  onRunUpdater?: () => void
  updaterStatus?: 'idle' | 'available' | 'checking' | 'downloading' | 'downloaded' | 'installing' | 'latest' | 'error'
  updaterLatestVersion?: string | null
  updaterBannerVisible?: boolean
  onDismissUpdaterBanner?: () => void
  appIconSrc: string
}

export interface AppShellProps {
  navbar: AppShellNavbarProps
  onWindowDrag: (event: { clientX: number; clientY: number }) => void
  onOnboardingComplete: () => void
  /** Subtrees the ContentRouter picks between. */
  router: Omit<ContentRouterProps, 'section' | 'onBackToChat'>
  /** App-global dialogs / drawers (permission prompt, telemetry
   * drawer, etc.) rendered inside the main shell regardless of
   * section. */
  overlays?: ReactNode
  /** Left-rail indicator rendered in the chat section only. */
  chatSectionOverlay?: ReactNode
}

/**
 * Compose BootShell + MainShell + ContentRouter into the
 * canonical App entry surface.
 */
export function AppShell({
  navbar,
  onWindowDrag,
  onOnboardingComplete,
  router,
  overlays,
  chatSectionOverlay,
}: AppShellProps) {
  const { phase } = useBootstrapState()

  // `BootShell` still consumes the 3-state `surface` prop the
  // legacy boot routing hook already produces. The bootstrap
  // store's `phase` is the canonical truth for splash /
  // onboarding / main / error transitions; the boot-route hook
  // adds the activation-gate layer on top.
  const bootRoute = useBootRoute({
    showSplash: phase === 'splash' || phase === 'error',
    showOnboarding: phase === 'onboarding',
  })
  const bootSurface = bootRouteToSurface(bootRoute)

  return (
    <BootShell
      surface={bootSurface}
      onWindowDrag={onWindowDrag}
      onOnboardingComplete={onOnboardingComplete}
    >
      <MainShell
        navbar={{
          activeSection: navbar.activeSection,
          onSelectSection: navbar.onSelectSection,
          onOpenSettings: navbar.onOpenSettings,
          onRunUpdater: navbar.onRunUpdater,
          updaterStatus: navbar.updaterStatus,
          updaterLatestVersion: navbar.updaterLatestVersion,
          updaterBannerVisible: navbar.updaterBannerVisible,
          onDismissUpdaterBanner: navbar.onDismissUpdaterBanner,
          onStartWindowDrag: onWindowDrag,
          appIconSrc: navbar.appIconSrc,
        }}
      >
        {navbar.activeSection === 'chat' ? chatSectionOverlay : null}
        <ContentRouter
          section={navbar.activeSection}
          onBackToChat={() => navbar.onSelectSection('chat')}
          chat={router.chat}
          memory={router.memory}
          sectionWorkspace={router.sectionWorkspace}
        />
        {overlays}
      </MainShell>
    </BootShell>
  )
}
