import { Activity, Clock3, Database, Hammer } from 'lucide-react'

import type { JiaochangViewModel } from '../data/jiaochang-types'
import type { JiaochangI18nKey } from '../i18n'

export function RunInspector({
  viewModel,
  t,
}: {
  viewModel: JiaochangViewModel
  t: (key: JiaochangI18nKey) => string
}) {
  const runningTools = viewModel.toolLedger.filter((entry) => entry.status === 'running').length

  return (
    <aside className="flex min-h-0 flex-col gap-4 rounded-[8px] border border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.72))] p-4 shadow-[var(--shadow-md)] backdrop-blur">
      <div>
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-[18px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{viewModel.title}</h2>
          <span
            className={`font-jiaochang-pixel rounded-full border px-2 py-0.5 text-[11px] font-semibold ${
              viewModel.source === 'fixture'
                ? 'border-amber-300 bg-amber-100 text-amber-900'
                : 'border-emerald-300 bg-emerald-100 text-emerald-900'
            }`}
          >
            {t(viewModel.source === 'fixture' ? 'badge.fixture' : 'badge.projection')}
          </span>
        </div>
        <p className="mt-2 text-[12px] leading-5 text-[var(--jiaochang-muted,#665a4d)]">
          {viewModel.adapterHealth.detail}
        </p>
      </div>

      <div className="grid grid-cols-2 gap-2">
        <Metric icon={Activity} label={t('panel.progress')} value={`${viewModel.runProgress.percent ?? 0}%`} />
        <Metric icon={Hammer} label={t('panel.tools')} value={`${runningTools}/${viewModel.toolLedger.length}`} />
        <Metric icon={Database} label={t('panel.source')} value={viewModel.source} />
        <Metric icon={Clock3} label={t('panel.phase')} value={t(`status.${viewModel.runProgress.phase}`)} />
      </div>

      <div className="rounded-[6px] border border-[var(--jiaochang-border,rgba(43,34,24,0.08))] bg-[var(--jiaochang-card-bg,#fffaf0)] p-3">
        <div className="flex items-center justify-between gap-3">
          <span className="text-[12px] font-semibold text-[var(--jiaochang-text,#2b2218)]">
            {viewModel.runProgress.currentStep ?? t('panel.currentStep.empty')}
          </span>
          <span className="text-[11px] text-[var(--jiaochang-muted,#7a5c3b)]">
            {viewModel.runProgress.completedSteps ?? 0}/{viewModel.runProgress.totalSteps ?? '?'}
          </span>
        </div>
        <div className="mt-3 h-2 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full bg-[var(--jiaochang-accent,#b9462f)]"
            style={{ width: `${Math.min(Math.max(viewModel.runProgress.percent ?? 0, 0), 100)}%` }}
          />
        </div>
      </div>

      <div>
        <h3 className="text-[13px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{t('panel.agents')}</h3>
        <div className="mt-2 space-y-2">
          {viewModel.agents.map((agent) => (
            <div key={agent.id} className="rounded-[6px] border border-[var(--jiaochang-border,rgba(43,34,24,0.08))] bg-[var(--jiaochang-card-bg,#fffaf0)] p-3">
              <div className="flex items-center justify-between gap-3">
                <span className="text-[13px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{agent.name}</span>
                <span className="text-[11px] text-[var(--jiaochang-muted,#7a5c3b)]">{agent.lane}</span>
              </div>
              <p className="mt-1 text-[12px] leading-5 text-[var(--jiaochang-muted,#665a4d)]">{agent.currentTask}</p>
            </div>
          ))}
        </div>
      </div>

      <div>
        <h3 className="text-[13px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{t('panel.toolLedger')}</h3>
        <div className="mt-2 space-y-2">
          {viewModel.toolLedger.map((entry) => (
            <div key={entry.id} className="rounded-[6px] border border-[var(--jiaochang-border,rgba(43,34,24,0.08))] bg-[var(--jiaochang-card-bg,#fffaf0)] p-3">
              <div className="flex items-center justify-between gap-3">
                <span className="truncate text-[12px] font-semibold text-[var(--jiaochang-text,#2b2218)]">
                  {entry.toolName}
                </span>
                <span className="rounded-full border border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.7))] px-2 py-0.5 text-[11px] text-[var(--jiaochang-muted,#7a5c3b)]">
                  {entry.status}
                </span>
              </div>
              {entry.summary ? (
                <p className="mt-1 text-[12px] leading-5 text-[var(--jiaochang-muted,#665a4d)]">{entry.summary}</p>
              ) : null}
            </div>
          ))}
        </div>
      </div>
    </aside>
  )
}

function Metric({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Activity
  label: string
  value: string
}) {
  return (
    <div className="rounded-[6px] border border-[var(--jiaochang-border,rgba(43,34,24,0.08))] bg-[var(--jiaochang-card-bg,#fffaf0)] p-3">
      <div className="flex items-center gap-2 text-[11px] font-semibold text-[var(--jiaochang-muted,#7a5c3b)]">
        <Icon className="h-3.5 w-3.5" />
        {label}
      </div>
      <div className="mt-1 truncate text-[16px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{value}</div>
    </div>
  )
}
