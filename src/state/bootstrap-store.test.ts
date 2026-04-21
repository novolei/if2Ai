// MIG-013 — bootstrap store + boot orchestrator tests.
//
// Runs under Node's built-in `node --test` runner via the
// MIG-012 path-alias loader (`scripts/test-loader.mjs`). No
// test-runner dependency is added to package.json.

import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import {
  createBootstrapStore,
  INITIAL_BOOTSTRAP_STATE,
  type BootstrapState,
} from './bootstrap-store.ts'
import { runBootSequence, type BootDependencies } from '@/boot/boot-orchestrator'

describe('bootstrap-store — state transitions', () => {
  it('initial snapshot matches INITIAL_BOOTSTRAP_STATE', () => {
    const store = createBootstrapStore()
    assert.deepEqual(store.getSnapshot(), INITIAL_BOOTSTRAP_STATE)
  })

  it('enterOnboarding transitions phase to "onboarding"', () => {
    const store = createBootstrapStore()
    store.enterOnboarding()
    assert.equal(store.getSnapshot().phase, 'onboarding')
  })

  it('bootReady populates projects + activeProjectId and phase == "main"', () => {
    const store = createBootstrapStore()
    store.bootReady({
      projects: [{ id: 'p1', name: 'P1', workdir: '/tmp/p1', created_at: '2024-01-01', session_count: 0 }],
      projectSessions: { p1: [] },
      activeProjectId: 'p1',
      currentProject: {
        id: 'p1',
        name: 'P1',
        workdir: '/tmp/p1',
        created_at: '2024-01-01',
        updated_at: '',
      },
    })
    const snap = store.getSnapshot()
    assert.equal(snap.phase, 'main')
    assert.equal(snap.activeProjectId, 'p1')
    assert.equal(snap.projects.length, 1)
    assert.ok(snap.currentProject)
    assert.equal(snap.startupError, null)
  })

  it('bootFailed sets phase "error" + captures message', () => {
    const store = createBootstrapStore()
    store.bootFailed('boom')
    const snap = store.getSnapshot()
    assert.equal(snap.phase, 'error')
    assert.equal(snap.startupError, 'boom')
  })

  it('onboardingComplete resets the store to the initial splash state', () => {
    const store = createBootstrapStore()
    store.enterOnboarding()
    store.onboardingComplete()
    assert.equal(store.getSnapshot().phase, 'splash')
    assert.equal(store.getSnapshot().projects.length, 0)
  })

  it('selectProject updates activeProjectId and currentProject', () => {
    const store = createBootstrapStore()
    store.selectProject({
      projectId: 'alpha',
      currentProject: {
        id: 'alpha',
        name: 'Alpha',
        workdir: '/tmp/alpha',
        created_at: '2024-01-01',
        updated_at: '',
      },
    })
    assert.equal(store.getSnapshot().activeProjectId, 'alpha')
    assert.equal(store.getSnapshot().currentProject?.name, 'Alpha')
  })

  it('subscribers are notified on every state replacement', () => {
    const store = createBootstrapStore()
    let calls = 0
    const unsubscribe = store.subscribe(() => {
      calls += 1
    })
    store.enterOnboarding()
    store.bootFailed('x')
    unsubscribe()
    store.onboardingComplete() // should NOT fire the unsubscribed listener
    assert.equal(calls, 2)
  })
})

describe('boot-orchestrator — runBootSequence', () => {
  function mockDeps(overrides: Partial<BootDependencies> = {}): BootDependencies {
    return {
      getOnboardingState: async () => ({ state: 'ready' }) as unknown as Record<string, unknown>,
      ensureDefaultWorkdir: async () => ['/tmp/wd', 'default-project'] as [string, string],
      listProjects: async () => [
        { id: 'default-project', name: 'Default', workdir: '/tmp/wd', created_at: '2024-01-01', session_count: 0 },
      ],
      listProjectSessions: async () => [],
      splashHoldMs: 0,
      onboardingTimeoutMs: 200,
      ...overrides,
    }
  }

  it('happy path reaches bootReady with the default project selected', async () => {
    const store = createBootstrapStore()
    const outcome = await runBootSequence(store, mockDeps())
    assert.equal(outcome, 'ready')
    const snap: BootstrapState = store.getSnapshot()
    assert.equal(snap.phase, 'main')
    assert.equal(snap.activeProjectId, 'default-project')
    assert.equal(snap.projects.length, 1)
  })

  it('first_launch state routes to onboarding', async () => {
    const store = createBootstrapStore()
    const outcome = await runBootSequence(
      store,
      mockDeps({
        getOnboardingState: async () => ({ state: 'first_launch' }) as unknown as Record<
          string,
          unknown
        >,
      }),
    )
    assert.equal(outcome, 'onboarding')
    assert.equal(store.getSnapshot().phase, 'onboarding')
  })

  it('listProjects throwing sets phase "error"', async () => {
    const store = createBootstrapStore()
    const outcome = await runBootSequence(
      store,
      mockDeps({
        listProjects: async () => {
          throw new Error('network down')
        },
      }),
    )
    assert.equal(outcome, 'error')
    const snap = store.getSnapshot()
    assert.equal(snap.phase, 'error')
    assert.ok(snap.startupError?.includes('network down'))
  })

  it('cancelled signal aborts without mutating the store', async () => {
    const store = createBootstrapStore()
    const signal = { cancelled: false }
    const p = runBootSequence(store, mockDeps({ splashHoldMs: 20, signal }))
    signal.cancelled = true
    await p
    // phase stays 'splash' because bootReady was not called
    assert.equal(store.getSnapshot().phase, 'splash')
  })

  it('onboarding-check timeout defaults to "not onboarding" and boots normally', async () => {
    const store = createBootstrapStore()
    const outcome = await runBootSequence(
      store,
      mockDeps({
        getOnboardingState: () => new Promise(() => {}), // never resolves
        onboardingTimeoutMs: 10,
      }),
    )
    assert.equal(outcome, 'ready')
    assert.equal(store.getSnapshot().phase, 'main')
  })
})
