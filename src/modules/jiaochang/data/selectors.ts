import { createJiaochangFixtureViewModel } from './fixture-adapter.ts'
import {
  createJiaochangProjectionViewModel,
  type JiaochangProjectionSnapshot,
} from './runtime-adapter.ts'
import type {
  JiaochangAgent,
  JiaochangPathReplay,
  JiaochangRunProgress,
  JiaochangToolLedgerEntry,
  JiaochangViewModel,
} from './jiaochang-types.ts'

export function getJiaochangViewModel(
  projectionSnapshot?: JiaochangProjectionSnapshot,
): JiaochangViewModel {
  if (projectionSnapshot) {
    return createJiaochangProjectionViewModel(projectionSnapshot)
  }
  return createJiaochangFixtureViewModel()
}

export function getSubagentIdentities(viewModel = getJiaochangViewModel()) {
  return viewModel.agents
    .map((agent) => agent.projectionIdentity)
    .filter((identity): identity is NonNullable<JiaochangAgent['projectionIdentity']> =>
      Boolean(identity),
    )
}

export function getToolLedger(
  viewModel = getJiaochangViewModel(),
): JiaochangToolLedgerEntry[] {
  return viewModel.toolLedger
}

export function getRunProgress(viewModel = getJiaochangViewModel()): JiaochangRunProgress {
  return viewModel.runProgress
}

export function getPathReplay(viewModel = getJiaochangViewModel()): JiaochangPathReplay {
  return viewModel.pathReplay
}
