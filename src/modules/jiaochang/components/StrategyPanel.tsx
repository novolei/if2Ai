import { Lightbulb } from 'lucide-react'

import type { JiaochangViewModel } from '../data/jiaochang-types.ts'
import { getJiaochangStrategyItems } from '../data/strategy-adapter.ts'
import type { JiaochangI18nKey } from '../i18n'

export function StrategyPanel({
  viewModel,
  t,
}: {
  viewModel: JiaochangViewModel
  t: (key: JiaochangI18nKey) => string
}) {
  const items = getJiaochangStrategyItems(viewModel)

  return (
    <section className="rounded-[8px] border border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.7))] p-4 shadow-[var(--shadow-sm)] backdrop-blur">
      <div className="flex items-center gap-2">
        <Lightbulb className="h-4 w-4 text-[var(--jiaochang-accent,#b9462f)]" />
        <h2 className="text-[14px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{t('strategy.title')}</h2>
      </div>
      <div className="mt-3 grid gap-2 lg:grid-cols-2">
        {items.map((item) => (
          <article key={item.id} className="rounded-[6px] border border-[var(--jiaochang-border,rgba(43,34,24,0.08))] bg-[var(--jiaochang-card-bg,#fffaf0)] p-3">
            <div className="flex items-center justify-between gap-2">
              <span className="text-[12px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{t(item.titleKey)}</span>
              {item.inferred ? (
                <span className="rounded-full border border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.7))] px-2 py-0.5 text-[11px] text-[var(--jiaochang-muted,#7a5c3b)]">
                  {t('strategy.inferred')}
                </span>
              ) : null}
            </div>
            <p className="mt-1 text-[12px] leading-5 text-[var(--jiaochang-muted,#665a4d)]">{t(item.bodyKey)}</p>
            <div className="mt-2 space-y-1">
              {item.evidence.map((evidence) => (
                <div key={evidence} className="truncate text-[11px] text-[var(--jiaochang-muted,#8a745a)]">
                  {evidence}
                </div>
              ))}
            </div>
          </article>
        ))}
      </div>
    </section>
  )
}
