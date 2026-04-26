import type { JiaochangI18nKey } from '../i18n/locales.ts'
import type { JiaochangToolLedgerEntry, JiaochangViewModel } from './jiaochang-types.ts'

export interface JiaochangStrategyItem {
  id: string
  titleKey: JiaochangI18nKey
  bodyKey: JiaochangI18nKey
  evidence: string[]
  inferred: boolean
  severity: 'info' | 'warning' | 'success'
}

export function getJiaochangStrategyItems(viewModel: JiaochangViewModel): JiaochangStrategyItem[] {
  const failedTools = viewModel.toolLedger.filter((entry) => entry.status === 'failed')
  const latestEvent = viewModel.events.at(-1)

  if (viewModel.runProgress.phase === 'blocked') {
    return [
      {
        id: 'blocked',
        titleKey: 'strategy.blocked.title',
        bodyKey: 'strategy.blocked.body',
        evidence: compactEvidence([
          failedTools[0] ? toolEvidence(failedTools[0]) : undefined,
          latestEvent?.title,
          viewModel.runProgress.currentStep,
        ]),
        inferred: true,
        severity: 'warning',
      },
    ]
  }

  if (viewModel.runProgress.phase === 'done') {
    return [
      {
        id: 'done',
        titleKey: 'strategy.done.title',
        bodyKey: 'strategy.done.body',
        evidence: compactEvidence([
          latestEvent?.title,
          `${viewModel.toolLedger.length} tool ledger entries`,
          viewModel.runProgress.currentStep,
        ]),
        inferred: true,
        severity: 'success',
      },
    ]
  }

  if (viewModel.agents.length === 0) {
    return [
      {
        id: 'empty',
        titleKey: 'strategy.empty.title',
        bodyKey: 'strategy.empty.body',
        evidence: [viewModel.adapterHealth.detail],
        inferred: true,
        severity: 'info',
      },
    ]
  }

  return [
    {
      id: 'running',
      titleKey: 'strategy.running.title',
      bodyKey: 'strategy.running.body',
      evidence: compactEvidence([
        viewModel.runProgress.currentStep,
        latestEvent?.title,
        `${viewModel.toolLedger.filter((entry) => entry.status === 'running').length} running tools`,
      ]),
      inferred: true,
      severity: 'info',
    },
  ]
}

function toolEvidence(entry: JiaochangToolLedgerEntry) {
  return `${entry.toolName}: ${entry.status}${entry.errorCode ? ` (${entry.errorCode})` : ''}`
}

function compactEvidence(items: Array<string | undefined>) {
  return items.filter((item): item is string => Boolean(item?.trim()))
}
