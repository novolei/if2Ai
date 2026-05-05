/**
 * GF-01 PR-03 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Renders the structured `/skills` slash-report card. The parser
 * {@link parseSkillsSlashReport} converts the legacy emoji-rich text
 * payload into a typed shape, and {@link SkillsSlashReport} draws the
 * grouped, source-aware list. {@link MarkdownContent} short-circuits to
 * this component when its body matches the report header.
 *
 * Render-equivalent move — props/behaviour unchanged from the original
 * inline definition.
 */

import * as React from "react"
import { FolderOpen, House, Lock, Sparkles, UserRound } from "lucide-react"
import { cn } from "@/lib/utils"

/** Mirrors the `DensityMode` union in `chat-ui.tsx`. */
type DensityMode = 'comfortable' | 'compact'

/** Parsed shape of a `/skills` slash-report payload. */
export type ParsedSkillsReport = {
  total: number
  enabled: number
  items: Array<{
    name: string
    sourceKey: string
    sourceLabel: string
    enabled: boolean
    status: string
    shadowedBy?: string
  }>
}

type GroupedSkillsReportItem = {
  name: string
  variants: ParsedSkillsReport['items']
}

/** Render the grouped skills report card. */
export function SkillsSlashReport({
  report,
  densityMode,
}: {
  report: ParsedSkillsReport
  densityMode: DensityMode
}) {
  const groupedItems = React.useMemo(() => groupSkillsReportItems(report.items), [report.items])

  return (
    <div className={cn('my-2.5 space-y-3.5', densityMode === 'compact' ? 'text-[12px]' : 'text-[13px]')}>
      <div className="flex items-center gap-2 text-foreground/82">
        <div className="flex size-7 items-center justify-center rounded-[6px] bg-muted text-muted-foreground">
          <Sparkles className="size-4" />
        </div>
        <div className="flex items-baseline gap-2">
          <span className="font-medium tracking-tight">Skills</span>
          <span className="text-muted-foreground">{report.total} total</span>
          <span className="text-muted-foreground/60">/</span>
          <span className="text-muted-foreground">{report.enabled} enabled</span>
        </div>
      </div>

      <div className="space-y-3">
        {groupedItems.map((group) => {
          const primary = group.variants[0]
          const SourceGlyph = getSkillSourceGlyph(primary.sourceKey)
          const isActive = primary.enabled && (primary.status === 'active' || primary.status === 'review_passed')

          return (
            <div key={`${group.name}-${group.variants.length}`} className="flex gap-3">
              <div className="flex w-5 shrink-0 justify-center pt-1">
                <span className={cn(
                  'size-2 rounded-full',
                  isActive ? 'bg-emerald-500/80' : primary.enabled ? 'bg-amber-400/85' : 'bg-muted-foreground/35'
                )} />
              </div>
              <div className="min-w-0 flex-1 space-y-1.5">
                <div className="flex min-w-0 items-center gap-2">
                  <div className="truncate font-medium tracking-tight text-foreground/82">{group.name}</div>
                  <span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-[10.5px] text-muted-foreground">
                    <SourceGlyph className="size-3" />
                    <span>{primary.sourceLabel}</span>
                  </span>
                  {group.variants.length > 1 ? (
                    <span className="shrink-0 text-[10.5px] text-muted-foreground">{group.variants.length} variants</span>
                  ) : null}
                </div>
                <div className="space-y-1">
                  {group.variants.map((variant, index) => (
                    <SkillMetaRow
                      key={`${group.name}-${variant.sourceKey}-${variant.shadowedBy ?? 'active'}-${index}`}
                      item={variant}
                      showSource={group.variants.length > 1}
                    />
                  ))}
                </div>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

/** Render a single variant row beneath a skill group. */
function SkillMetaRow({
  item,
  showSource,
}: {
  item: ParsedSkillsReport['items'][number]
  showSource: boolean
}) {
  const SourceGlyph = getSkillSourceGlyph(item.sourceKey)

  return (
    <div className="flex min-w-0 items-start gap-2 text-[11px] text-muted-foreground">
      {showSource ? (
        <span className="inline-flex shrink-0 items-center gap-1 text-muted-foreground">
          <SourceGlyph className="size-3" />
          <span>{item.sourceLabel}</span>
        </span>
      ) : (
        <span className={cn('mt-[4px] size-1.5 shrink-0 rounded-full', item.enabled ? 'bg-emerald-500/55' : 'bg-muted-foreground/35')} />
      )}
      <div className="min-w-0 truncate">
        <span>{item.enabled ? 'enabled' : 'disabled'}</span>
        <span className="px-1 text-muted-foreground/55">·</span>
        <span>
          <span className="text-muted-foreground">status </span>
          <span className="font-mono italic text-foreground/72">{item.status}</span>
        </span>
        {item.shadowedBy ? (
          <>
            <span className="px-1 text-muted-foreground/55">·</span>
            <span>
              <span className="text-muted-foreground">shadowed by </span>
              <span className="font-mono italic text-foreground/72">{item.shadowedBy}</span>
            </span>
          </>
        ) : null}
      </div>
    </div>
  )
}

/** Group `items` by skill name and order each group by `getSkillVariantPriority`. */
export function groupSkillsReportItems(items: ParsedSkillsReport['items']): GroupedSkillsReportItem[] {
  const groups = new Map<string, GroupedSkillsReportItem>()

  for (const item of items) {
    const key = item.name.toLowerCase()
    const existing = groups.get(key)
    if (existing) {
      existing.variants.push(item)
    } else {
      groups.set(key, { name: item.name, variants: [item] })
    }
  }

  for (const group of groups.values()) {
    group.variants.sort((a, b) => getSkillVariantPriority(b) - getSkillVariantPriority(a))
  }

  return Array.from(groups.values())
}

/** Score a variant so higher-priority (active, unshadowed, workspace) sort first. */
function getSkillVariantPriority(item: ParsedSkillsReport['items'][number]) {
  let score = 0
  if (item.enabled) score += 4
  if (item.status === 'active' || item.status === 'review_passed') score += 3
  if (!item.shadowedBy) score += 2
  if (item.sourceKey === 'workspace') score += 1.5
  else if (item.sourceKey === 'user') score += 1
  else if (item.sourceKey === 'builtin') score += 0.5
  return score
}

/** Parse the legacy `📋 Skills (N total, M enabled)` text payload into a typed report, or `null`. */
export function parseSkillsSlashReport(content: string): ParsedSkillsReport | null {
  const normalized = content.replace(/\r\n/g, '\n').trim()
  const lines = normalized.split('\n').map((line) => line.trimEnd())
  const header = lines[0]?.match(/^📋\s+Skills\s+\((\d+)\s+total,\s+(\d+)\s+enabled\)$/)
  if (!header) return null

  const items: ParsedSkillsReport['items'] = []
  let current: ParsedSkillsReport['items'][number] | null = null

  for (const rawLine of lines.slice(2)) {
    const line = rawLine.trim()
    if (!line) continue

    const itemMatch = line.match(/^(🟢|⚪)(✅|🔒|📝|❌|⚠️)\s+(.+?)\s+(🏠 builtin|👤 user|📁 workspace|🔒 quarantine|[^\s]+)$/)
    if (itemMatch) {
      if (current) items.push(current)
      current = {
        enabled: itemMatch[1] === '🟢',
        status: normalizeSkillStatusFromEmoji(itemMatch[2]),
        name: itemMatch[3].trim(),
        sourceKey: normalizeSkillSourceKey(itemMatch[4]),
        sourceLabel: normalizeSkillSourceLabel(itemMatch[4]),
      }
      continue
    }

    if (!current) continue

    const metaMatch = line.match(/^└\s+(enabled|disabled)\s+\|\s+status:\s+(.+)$/)
    if (metaMatch) {
      current.enabled = metaMatch[1] === 'enabled'
      current.status = metaMatch[2].trim()
      continue
    }

    const shadowedMatch = line.match(/^└\s+shadowed by\s+(.+)$/)
    if (shadowedMatch) {
      current.shadowedBy = shadowedMatch[1].trim()
    }
  }

  if (current) items.push(current)
  if (!items.length) return null

  return {
    total: Number(header[1]),
    enabled: Number(header[2]),
    items,
  }
}

function normalizeSkillStatusFromEmoji(emoji: string) {
  if (emoji === '✅') return 'active'
  if (emoji === '🔒') return 'quarantine'
  if (emoji === '📝') return 'draft'
  if (emoji === '❌') return 'disabled'
  return 'warning'
}

function normalizeSkillSourceKey(sourceLabel: string) {
  if (sourceLabel.includes('builtin')) return 'builtin'
  if (sourceLabel.includes('user')) return 'user'
  if (sourceLabel.includes('workspace')) return 'workspace'
  if (sourceLabel.includes('quarantine')) return 'quarantine'
  return sourceLabel
}

function normalizeSkillSourceLabel(sourceLabel: string) {
  return sourceLabel
    .replace('🏠 ', '')
    .replace('👤 ', '')
    .replace('📁 ', '')
    .replace('🔒 ', '')
}

function getSkillSourceGlyph(sourceKey: string): React.ComponentType<{ className?: string }> {
  if (sourceKey === 'builtin') return House
  if (sourceKey === 'user') return UserRound
  if (sourceKey === 'workspace') return FolderOpen
  if (sourceKey === 'quarantine') return Lock
  return Sparkles
}
