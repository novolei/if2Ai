import type { JiaochangRunEvent, JiaochangRuntimeAnchor } from '../data/jiaochang-types'
import type { JiaochangI18nKey } from '../i18n'

export function TimelinePanel({
  events,
  onOpenAnchor,
  t,
}: {
  events: JiaochangRunEvent[]
  onOpenAnchor: (anchor: JiaochangRuntimeAnchor) => void
  t: (key: JiaochangI18nKey) => string
}) {
  return (
    <section className="rounded-[8px] border border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.7))] p-4 shadow-[var(--shadow-sm)] backdrop-blur">
      <h2 className="text-[14px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{t('timeline.title')}</h2>
      <div className="mt-3 grid gap-2 md:grid-cols-3">
        {events.map((event) => (
          <article key={event.id} className="rounded-[6px] border border-[var(--jiaochang-border,rgba(43,34,24,0.08))] bg-[var(--jiaochang-card-bg,#fffaf0)] p-3">
            <div className="flex items-center justify-between gap-2">
              <span className="text-[12px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{event.title}</span>
              <span className="text-[11px] text-[var(--jiaochang-muted,#8a745a)]">{event.ts}</span>
            </div>
            <p className="mt-1 text-[12px] leading-5 text-[var(--jiaochang-muted,#665a4d)]">{event.detail}</p>
            {event.anchor ? (
              <button
                type="button"
                className="font-jiaochang-pixel mt-2 text-[11px] font-semibold text-[#8b3a28] hover:underline"
                onClick={() => onOpenAnchor(event.anchor!)}
              >
                {t('anchor.open')} · {event.anchor.label}
              </button>
            ) : null}
          </article>
        ))}
      </div>
    </section>
  )
}
