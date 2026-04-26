import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { createJiaochangFixtureViewModel } from './fixture-adapter.ts'
import { applyReplayStepToAgents, createPathReplayFromEvents } from './path-replay.ts'

describe('jiaochang path replay', () => {
  it('builds fixture replay steps with anchors from typed events', () => {
    const viewModel = createJiaochangFixtureViewModel()

    assert.equal(viewModel.pathReplay.runId, viewModel.runId)
    assert.equal(viewModel.pathReplay.steps.length, viewModel.events.length)
    assert.ok(viewModel.pathReplay.steps.some((step) => step.anchor?.kind === 'chat'))
    assert.ok(viewModel.pathReplay.steps.some((step) => step.anchor?.kind === 'tool'))
    assert.ok(viewModel.pathReplay.steps.some((step) => step.anchor?.kind === 'file'))
  })

  it('applies a replay step without mutating non-target agents', () => {
    const viewModel = createJiaochangFixtureViewModel()
    const step = viewModel.pathReplay.steps[0]
    assert.ok(step)

    const projected = applyReplayStepToAgents(viewModel.agents, step)
    const changed = projected.find((agent) => agent.id === step.agentId)
    const unchanged = projected.find((agent) => agent.id !== step.agentId)

    assert.equal(changed?.status, step.status)
    assert.deepEqual(changed?.position, step.position)
    assert.equal(unchanged?.position, viewModel.agents.find((agent) => agent.id === unchanged?.id)?.position)
  })

  it('can derive replay from projection events when no explicit replay is supplied', () => {
    const viewModel = createJiaochangFixtureViewModel()
    const replay = createPathReplayFromEvents('run-derived', viewModel.events, viewModel.agents)

    assert.equal(replay.runId, 'run-derived')
    assert.equal(replay.steps[0]?.status, 'planning')
    assert.equal(replay.steps.at(-1)?.status, 'done')
  })
})
