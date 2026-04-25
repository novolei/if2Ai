import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from 'react'
import { createPortal } from 'react-dom'
import { Check, Sparkles } from 'lucide-react'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { getPersonaAvatarSrc } from '@/lib/persona-avatars'
import {
  getPromptControlCatalog,
  getPromptControlSettings,
  type PromptControlCatalog,
  type PromptControlPersonaOption,
  type PromptControlSettings,
  type SessionMeta,
} from '@/lib/tauri'
import type { SessionIdentityInput } from '@/api/identity'
import { useCrossWindowChange } from '@/lib/crossWindowSync'

interface SidebarTopProps {
  onStartWindowDrag: (event: ReactMouseEvent<HTMLElement>) => void
  onNewThread: () => void
  activeSessionId?: string | null
  activeSessionMeta?: SessionMeta | null
  activeTitle?: string
  onUpdateSessionIdentity?: (
    sessionId: string,
    identity: SessionIdentityInput,
  ) => Promise<SessionMeta | void>
}

function resolveAgentDisplayName(settings: PromptControlSettings | null): string {
  const agentName = settings?.agent_name?.trim()
  if (agentName) return agentName
  return 'If2Ai'
}

/**
 * Resolve which persona is currently in effect for the agent badge.
 *
 * Precedence mirrors the backend resolver:
 *   session override > global default > built-in (null).
 *
 * For the sidebar header we surface the live state so the avatar always
 * matches what the LLM is actually using right now.
 */
function resolveActivePersonaId(
  settings: PromptControlSettings | null,
  session: SessionMeta | null | undefined,
): string | null {
  if (session?.persona_id) return session.persona_id
  return settings?.default_persona_id ?? null
}

export function SidebarTop({
  onStartWindowDrag,
  onNewThread,
  activeSessionId,
  activeSessionMeta,
  activeTitle,
  onUpdateSessionIdentity,
}: SidebarTopProps) {
  const [settings, setSettings] = useState<PromptControlSettings | null>(null)
  const [catalog, setCatalog] = useState<PromptControlCatalog | null>(null)
  const [popoverOpen, setPopoverOpen] = useState(false)
  const [saving, setSaving] = useState(false)
  const [popoverPos, setPopoverPos] = useState<{
    top: number
    left: number
  } | null>(null)
  // Optimistic local view of the active session's identity. We patch
  // this immediately on persona switch so the avatar/title flip without
  // waiting for the parent's React state -> useMemo -> prop chain to
  // settle (which can race with React batching). Cleared / overridden
  // whenever a fresh `activeSessionMeta` prop arrives for a different
  // session, or when the upstream value catches up.
  const [optimisticIdentity, setOptimisticIdentity] = useState<{
    sessionId: string
    soul_id: string | null
    persona_id: string | null
  } | null>(null)
  const popoverRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)

  const POPOVER_WIDTH = 320
  const VIEWPORT_PADDING = 8

  const reload = async () => {
    try {
      const [nextSettings, nextCatalog] = await Promise.all([
        getPromptControlSettings(),
        getPromptControlCatalog(),
      ])
      setSettings(nextSettings)
      setCatalog(nextCatalog)
    } catch {
      // Best-effort: header stays on built-in defaults if backend is unhappy.
    }
  }

  useEffect(() => {
    void reload()
  }, [])

  useCrossWindowChange<unknown>('cross:session-identity-changed', () => {
    void reload()
  })
  useCrossWindowChange<unknown>('cross:prompt-control-changed', () => {
    void reload()
  })

  // Close popover on outside click / Escape.
  useEffect(() => {
    if (!popoverOpen) return
    const handleClick = (event: MouseEvent) => {
      const target = event.target as Node
      if (
        popoverRef.current?.contains(target) ||
        triggerRef.current?.contains(target)
      ) {
        return
      }
      setPopoverOpen(false)
    }
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setPopoverOpen(false)
    }
    document.addEventListener('mousedown', handleClick)
    document.addEventListener('keydown', handleKey)
    return () => {
      document.removeEventListener('mousedown', handleClick)
      document.removeEventListener('keydown', handleKey)
    }
  }, [popoverOpen])

  // Position the popover via portal so the parent sidebar's
  // `overflow-hidden` doesn't clip it. Recompute on open + on
  // window resize / scroll while open.
  useLayoutEffect(() => {
    if (!popoverOpen) {
      setPopoverPos(null)
      return
    }
    const compute = () => {
      const trigger = triggerRef.current
      if (!trigger) return
      const rect = trigger.getBoundingClientRect()
      const top = rect.bottom + 6
      const rawLeft = rect.left
      const maxLeft =
        window.innerWidth - POPOVER_WIDTH - VIEWPORT_PADDING
      const left = Math.max(VIEWPORT_PADDING, Math.min(rawLeft, maxLeft))
      setPopoverPos({ top, left })
    }
    compute()
    window.addEventListener('resize', compute)
    window.addEventListener('scroll', compute, true)
    return () => {
      window.removeEventListener('resize', compute)
      window.removeEventListener('scroll', compute, true)
    }
  }, [popoverOpen])

  // Discard stale optimistic patch when the active session changes,
  // or when upstream prop has caught up to the optimistic value.
  useEffect(() => {
    if (!optimisticIdentity) return
    if (optimisticIdentity.sessionId !== activeSessionId) {
      setOptimisticIdentity(null)
      return
    }
    if (
      activeSessionMeta &&
      (activeSessionMeta.persona_id ?? null) === optimisticIdentity.persona_id &&
      (activeSessionMeta.soul_id ?? null) === optimisticIdentity.soul_id
    ) {
      setOptimisticIdentity(null)
    }
  }, [activeSessionId, activeSessionMeta, optimisticIdentity])

  // Effective session meta = upstream prop with optimistic patch overlaid.
  const effectiveSessionMeta: SessionMeta | null | undefined =
    optimisticIdentity && optimisticIdentity.sessionId === activeSessionId
      ? activeSessionMeta
        ? {
            ...activeSessionMeta,
            soul_id: optimisticIdentity.soul_id,
            persona_id: optimisticIdentity.persona_id,
          }
        : null
      : activeSessionMeta

  const personaId = resolveActivePersonaId(settings, effectiveSessionMeta)
  const activePersona: PromptControlPersonaOption | null = personaId
    ? (catalog?.personas.find((p) => p.id === personaId) ?? null)
    : null
  const avatarSrc = getPersonaAvatarSrc(personaId, activePersona?.avatar_id)
  const agentName = resolveAgentDisplayName(settings)

  // Personas eligible for the *active session*: those under whichever
  // soul the session is using (session override soul, else default soul).
  const effectiveSoulId =
    effectiveSessionMeta?.soul_id ?? settings?.default_soul_id ?? null
  const personaChoices = (catalog?.personas ?? []).filter((persona) =>
    effectiveSoulId ? persona.soul_id === effectiveSoulId : true,
  )

  const canSwitch =
    Boolean(activeSessionId) && Boolean(onUpdateSessionIdentity) && !saving

  const handleSelectPersona = async (
    nextPersonaId: string | null,
  ) => {
    if (!activeSessionId || !onUpdateSessionIdentity) return
    const nextPersona = nextPersonaId
      ? (catalog?.personas.find((persona) => persona.id === nextPersonaId) ??
        null)
      : null
    const nextSoulId =
      nextPersona?.soul_id ?? effectiveSessionMeta?.soul_id ?? effectiveSoulId ?? null
    // Optimistic UI: flip avatar/title immediately so the user gets
    // instant feedback. The reconciliation effect above will retire
    // this patch as soon as the parent prop catches up (or when the
    // active session changes).
    setOptimisticIdentity({
      sessionId: activeSessionId,
      soul_id: nextSoulId,
      persona_id: nextPersonaId,
    })
    setSaving(true)
    try {
      const updated = await onUpdateSessionIdentity(activeSessionId, {
        // Keep whichever soul was already active on the session (or fall
        // back to the selected persona's owning soul) so the backend
        // prompt resolver sees the same persona the UI just showed.
        soul_id: nextSoulId,
        persona_id: nextPersonaId,
      })
      if (
        updated &&
        (updated.persona_id ?? null) !== nextPersonaId
      ) {
        throw new Error(
          `后端返回的 Persona 不匹配：expected=${nextPersonaId ?? 'global'}, actual=${
            updated.persona_id ?? 'global'
          }`,
        )
      }
      toast.success(
        nextPersonaId
          ? '已切换 Persona'
          : '已切换为跟随全局默认',
        {
          description: nextPersonaId
            ? `当前会话现在以 ${
                catalog?.personas.find((p) => p.id === nextPersonaId)?.name ??
                nextPersonaId
              } 模式协作`
            : '当前会话不再使用 session-level override',
        },
      )
      setPopoverOpen(false)
    } catch (error) {
      // Roll back the optimistic patch on failure so the UI doesn't
      // misrepresent the actual session state.
      setOptimisticIdentity(null)
      toast.error('切换 Persona 失败', { description: String(error) })
    } finally {
      setSaving(false)
    }
  }

  return (
    <div
      className="window-drag grid h-[60px] grid-cols-[minmax(0,1fr)_auto] items-center border-b border-black/[0.06] select-none"
      onMouseDown={onStartWindowDrag}
    >
      {/* ── Agent identity badge (clickable when a session is active) ── */}
      <div className="flex min-w-0 items-center gap-2.5 px-3">
        <button
          ref={triggerRef}
          type="button"
          data-window-no-drag="true"
          onClick={(event) => {
            event.stopPropagation()
            if (!canSwitch) return
            setPopoverOpen((value) => !value)
          }}
          aria-label="切换 Persona"
          aria-expanded={popoverOpen}
          className={cn(
            'window-no-drag group flex min-w-0 items-center gap-2.5 rounded-xl px-1 py-0.5 text-left transition-all',
            canSwitch
              ? 'cursor-pointer hover:bg-black/[0.04] active:scale-[0.98]'
              : 'cursor-default',
          )}
          disabled={!canSwitch}
        >
          {avatarSrc ? (
            <div
              className={cn(
                'relative size-11 shrink-0 overflow-hidden rounded-full border border-black/[0.08] bg-[#f5efe5] shadow-[0_2px_8px_rgba(15,23,42,0.08),inset_0_0_0_2px_rgba(255,255,255,0.7)] transition-transform',
                canSwitch && 'group-hover:scale-105',
              )}
            >
              <img
                src={avatarSrc}
                alt={activePersona?.name ?? agentName}
                className="h-full w-full object-cover object-[50%_30%] select-none"
                draggable={false}
              />
            </div>
          ) : (
            <div
              className={cn(
                'flex size-11 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-jade/80 to-jade shadow-[0_2px_8px_rgba(16,185,129,0.25)] transition-transform',
                canSwitch && 'group-hover:scale-105',
              )}
            >
              <Sparkles className="size-4 text-white" strokeWidth={1.5} />
            </div>
          )}

          <div className="flex min-w-0 flex-col justify-center gap-0">
            <span
              className="truncate text-[14.5px] font-bold tracking-tight text-foreground/88"
              title={agentName}
            >
              {agentName}
            </span>
            <span
              className="truncate text-[10px] font-medium tracking-wider text-muted-foreground/55"
              title={activePersona?.name ?? '智能助理'}
            >
              {activePersona?.name ?? '智能助理'}
            </span>
          </div>
        </button>
      </div>

      {/* ── New chat button ─────────────────────────── */}
      <div className="pr-3">
        <button
          type="button"
          data-window-no-drag="true"
          onClick={onNewThread}
          className="window-no-drag group flex h-7 cursor-pointer items-center gap-1.5 rounded-lg bg-jade/10 pl-2.5 pr-3 text-[12px] font-semibold text-jade transition-all hover:bg-jade hover:text-white hover:shadow-sm hover:shadow-jade/25 active:scale-[0.97]"
        >
          <svg
            className="size-3 transition-transform group-hover:rotate-45 group-hover:scale-110"
            fill="none"
            stroke="currentColor"
            strokeWidth={2.5}
            viewBox="0 0 16 16"
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M8 3v10M3 8h10" />
          </svg>
          新聊天
        </button>
      </div>

      {/* ── Persona quick-switch popover (portal'd to body) ───
          The sidebar `<aside>` uses `overflow-hidden`, which would
          clip an in-flow popover. Render through a portal anchored
          via fixed positioning, recomputed on open / resize / scroll. */}
      {popoverOpen && canSwitch && popoverPos
        ? createPortal(
            <div
              ref={popoverRef}
              data-window-no-drag="true"
              onMouseDown={(event) => event.stopPropagation()}
              style={{
                position: 'fixed',
                top: popoverPos.top,
                left: popoverPos.left,
                width: POPOVER_WIDTH,
              }}
              className="window-no-drag z-[60] overflow-hidden rounded-2xl border border-black/[0.08] bg-white/98 shadow-[0_18px_48px_rgba(15,23,42,0.18)] backdrop-blur-md"
            >
              <div className="border-b border-black/[0.06] px-4 py-3">
                <div className="flex items-center justify-between gap-2">
                  <div className="text-[10.5px] font-semibold uppercase tracking-[0.18em] text-black/40">
                    Switch Persona
                  </div>
                  <span className="rounded-md bg-amber-50/80 px-1.5 py-0.5 text-[9.5px] font-semibold uppercase tracking-[0.14em] text-amber-700">
                    Session only
                  </span>
                </div>
                <p className="mt-1 text-[11px] leading-[1.55] text-muted-foreground">
                  仅切换
                  {activeTitle ? (
                    <span className="mx-0.5 rounded bg-black/[0.05] px-1 py-px text-[10.5px] text-foreground/75">
                      {activeTitle}
                    </span>
                  ) : (
                    '当前会话'
                  )}
                  的表达模式，不改全局默认。
                </p>
              </div>

              <div className="max-h-[60vh] overflow-y-auto px-2 py-2">
                <PopoverPersonaRow
                  active={!effectiveSessionMeta?.persona_id}
                  onClick={() => void handleSelectPersona(null)}
                  avatarSrc={null}
                  title="跟随全局默认"
                  subtitle={
                    settings?.default_persona_id && catalog
                      ? (catalog.personas.find(
                          (p) => p.id === settings.default_persona_id,
                        )?.name ?? '默认 Persona')
                      : 'Auto / 内建默认'
                  }
                  fallbackBadge="AUTO"
                />
                <div className="my-1.5 mx-2 h-px bg-black/[0.05]" />
                {personaChoices.length === 0 ? (
                  <div className="px-3 py-4 text-center text-[12px] text-muted-foreground">
                    当前 Soul 下暂无可选 Persona
                  </div>
                ) : (
                  personaChoices.map((persona) => (
                    <PopoverPersonaRow
                      key={persona.id}
                      active={effectiveSessionMeta?.persona_id === persona.id}
                      onClick={() => void handleSelectPersona(persona.id)}
                      avatarSrc={getPersonaAvatarSrc(persona.id, persona.avatar_id)}
                      title={persona.name}
                      subtitle={persona.summary}
                    />
                  ))
                )}
              </div>
            </div>,
            document.body,
          )
        : null}
    </div>
  )
}

function PopoverPersonaRow({
  active,
  onClick,
  avatarSrc,
  title,
  subtitle,
  fallbackBadge,
}: {
  active: boolean
  onClick: () => void
  avatarSrc: string | null
  title: string
  subtitle: string
  fallbackBadge?: string
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'group flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition-all',
        active
          ? 'bg-jade/[0.08]'
          : 'hover:bg-black/[0.03]',
      )}
    >
      {avatarSrc ? (
        <div className="relative size-10 shrink-0 overflow-hidden rounded-full border border-black/[0.08] bg-[#f5efe5] shadow-[inset_0_0_0_2px_rgba(255,255,255,0.7)]">
          <img
            src={avatarSrc}
            alt={title}
            className="h-full w-full object-cover object-[50%_30%] select-none"
            draggable={false}
          />
        </div>
      ) : (
        <div className="flex size-10 shrink-0 items-center justify-center rounded-full border border-dashed border-black/[0.12] bg-black/[0.025] text-[9.5px] font-semibold uppercase tracking-wider text-black/45">
          {fallbackBadge ?? '·'}
        </div>
      )}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-[12.5px] font-semibold tracking-tight text-foreground/88">
            {title}
          </span>
          {active ? (
            <Check className="h-3 w-3 shrink-0 text-jade" />
          ) : null}
        </div>
        <div className="mt-0.5 line-clamp-2 text-[11px] leading-4 text-muted-foreground">
          {subtitle}
        </div>
      </div>
    </button>
  )
}
