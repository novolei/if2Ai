import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { JiaochangAudioEventBus } from './audio-events.ts'

describe('JiaochangAudioEventBus', () => {
  it('publishes and unsubscribes typed audio events', () => {
    const bus = new JiaochangAudioEventBus()
    const received: string[] = []
    const unsubscribe = bus.subscribe('track:started', (payload) => {
      if (payload.trackId) received.push(payload.trackId)
    })

    bus.emit('track:started', { trackId: 'track-1' })
    unsubscribe()
    bus.emit('track:started', { trackId: 'track-2' })

    assert.deepEqual(received, ['track-1'])
  })
})
