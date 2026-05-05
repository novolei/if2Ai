/**
 * GF-01 PR-07 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Visual section header inside the project-files rail: tiny eyebrow
 * label plus a count chip. Pure presentational; no state. Render-
 * equivalent move from the original inline definition.
 */

import * as React from "react"

/**
 * Render a labelled, counted group section inside the project-files rail.
 * Used to bracket sub-lists (e.g. recent files) underneath the breadcrumb
 * chrome. `children` is the list body itself.
 */
export function ProjectRailGroup({
  title,
  count,
  children,
}: {
  title: string
  count: number
  children: React.ReactNode
}) {
  return (
    <section className="mb-2">
      <div className="mb-1.5 flex items-center justify-between px-1">
        <div className="text-[10px] uppercase tracking-[0.22em] text-muted-foreground/50">{title}</div>
        <div className="text-[10px] text-muted-foreground/40">{count}</div>
      </div>
      <div className="space-y-0.5">{children}</div>
    </section>
  )
}
