import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { createJiaochangFixtureViewModel } from './fixture-adapter.ts'
import { getJiaochangStrategyItems } from './strategy-adapter.ts'

describe('jiaochang strategy adapter', () => {
  it('returns evidence-backed running guidance for active runs', () => {
    const items = getJiaochangStrategyItems(createJiaochangFixtureViewModel())

    assert.equal(items[0]?.titleKey, 'strategy.running.title')
    assert.equal(items[0]?.inferred, true)
    assert.ok((items[0]?.evidence.length ?? 0) >= 1)
  })

  it('returns blocked and done guidance from run phase', () => {
    const blocked = createJiaochangFixtureViewModel()
    blocked.runProgress.phase = 'blocked'
    blocked.toolLedger[0] = {
      ...blocked.toolLedger[0],
      status: 'failed',
      errorCode: 'tool_failed',
    }

    const done = createJiaochangFixtureViewModel()
    done.runProgress.phase = 'done'

    assert.equal(getJiaochangStrategyItems(blocked)[0]?.titleKey, 'strategy.blocked.title')
    assert.equal(getJiaochangStrategyItems(blocked)[0]?.severity, 'warning')
    assert.equal(getJiaochangStrategyItems(done)[0]?.titleKey, 'strategy.done.title')
    assert.equal(getJiaochangStrategyItems(done)[0]?.severity, 'success')
  })
})
