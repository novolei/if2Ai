import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import {
  getJiaochangViewModel,
  getPathReplay,
  getRunProgress,
  getSubagentIdentities,
  getToolLedger,
} from './selectors.ts'

describe('jiaochang fixture projection selectors', () => {
  it('exposes projection-ready subagent identity, tool ledger and run progress', () => {
    const viewModel = getJiaochangViewModel()

    assert.equal(viewModel.source, 'fixture')
    assert.equal(getSubagentIdentities(viewModel).length, 3)
    assert.ok(getSubagentIdentities(viewModel).some((identity) => identity.role === 'subagent' || identity.role === 'reviewer'))
    assert.equal(getRunProgress(viewModel).phase, 'executing')
    assert.equal(typeof getRunProgress(viewModel).percent, 'number')
    assert.ok(getToolLedger(viewModel).some((entry) => entry.status === 'running'))
    assert.ok(getPathReplay(viewModel).steps.length >= 1)
  })

  it('maps a runtime projection snapshot onto the same jiaochang view model', () => {
    const viewModel = getJiaochangViewModel({
      title: '真实投影运行',
      runId: 'run-real-1',
      updatedAt: '2026-04-25T12:00:00.000Z',
      runProgress: {
        runId: 'run-real-1',
        phase: 'writing',
        percent: 80,
        currentStep: '写入结果摘要',
      },
      subagentIdentities: [
        {
          agentId: 'main-real',
          displayName: '主控',
          role: 'main',
          runId: 'run-real-1',
        },
      ],
      toolLedger: [
        {
          id: 'tool-real-1',
          runId: 'run-real-1',
          agentId: 'main-real',
          toolName: 'write',
          status: 'succeeded',
        },
      ],
      events: [
        {
          id: 'evt-real-1',
          ts: '12:00',
          agentId: 'main-real',
          type: 'done',
          title: '生成摘要',
        },
      ],
    })

    assert.equal(viewModel.source, 'projection')
    assert.equal(viewModel.adapterHealth.status, 'ready')
    assert.equal(viewModel.agents[0]?.status, 'writing')
    assert.equal(viewModel.agents[0]?.projectionIdentity?.agentId, 'main-real')
    assert.equal(getToolLedger(viewModel)[0]?.toolName, 'write')
    assert.equal(getPathReplay(viewModel).steps[0]?.anchor?.kind, undefined)
  })
})
