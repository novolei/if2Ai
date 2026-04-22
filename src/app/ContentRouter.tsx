// MIG-013 — ContentRouter.
//
// Single home for the "which main-surface component should render
// for the given `activeSection`?" decision that used to live as
// an inline ternary chain inside `App.tsx`. Every surface the
// router mounts is passed as an already-constructed element /
// render prop so this file stays pure (no `src/api/*` imports,
// no project / session state knowledge).
//
// Future surfaces (specialized agents, settings-inline, etc.)
// extend the router by adding a new branch here; upstream
// callers never learn the mapping.

import type { ReactNode } from 'react'

import type { AppSection } from '@/modules/app-shell/types'

/** Props the router needs from `AppShell`. Each field is the
 * already-rendered surface for the corresponding `AppSection`;
 * the router only picks one. */
export interface ContentRouterProps {
  /** Active section the navbar (or an external state store)
   * has selected. */
  section: AppSection
  /** Handler the non-chat, non-memory surfaces call to return
   * to the chat workspace. */
  onBackToChat: () => void
  /** Chat workspace subtree (projects, sessions, composer). */
  chat: ReactNode
  /** Memory browser subtree. */
  memory: ReactNode
  /** Section-workspace fallback used for every other
   * `AppSection` the chat / memory surfaces don't handle. */
  sectionWorkspace: (props: { section: AppSection; onBackToChat: () => void }) => ReactNode
}

export function ContentRouter({
  section,
  onBackToChat,
  chat,
  memory,
  sectionWorkspace,
}: ContentRouterProps) {
  if (section === 'chat') return <>{chat}</>
  if (section === 'memory') return <>{memory}</>
  return <>{sectionWorkspace({ section, onBackToChat })}</>
}
