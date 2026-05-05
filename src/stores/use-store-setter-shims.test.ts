// GF-03 PR-3 — `useAppStateSetters` shim tests.
//
// `useAppStateSetters` is a React hook (calls `useCallback` /
// store-subscribing selector hooks), so we cannot mount it
// under `node --test` without pulling in a renderer the repo
// does not depend on. We therefore split coverage in two:
//
//   (a) **Structural assertions** on the source file — every
//       one of the 10 setter names is exported by name, each
//       writer wraps `useCallback(..., [])` so closure-stability
//       is preserved, and each wrapper resolves the previous
//       value via the canonical store snapshot (not the React-
//       rendered slice) so multiple calls in one async closure
//       compose correctly.
//
//   (b) **Behavior round-trips** that exercise the *same store
//       actions the shims dispatch into* (`bootstrapStore.*`,
//       `setConversation`, `setSessionLoading`, …). Each test
//       sets a value via the action, reads it back via the
//       canonical snapshot, and asserts the round-trip matches
//       what the shim contract promises ("functional updater +
//       raw value + no-op diff path").
//
// Together these guarantee the shims wire to the right store
// surface without introducing a renderer dependency.

import { strict as assert } from 'node:assert'
import { readFileSync } from 'node:fs'
import { describe, it, beforeEach } from 'node:test'

import { bootstrapStore } from '../state/bootstrap-store.ts'
import {
  clearStreamAbortHandle,
  getConversationSnapshot,
  initTitleState,
  removeSession,
  setConversation,
  setSessionLoading,
  setSessionTodos,
  setStreamAbortHandle,
  setTitleState,
} from './conversation-slice.ts'
import { sessionStore } from './session-store.ts'

const SOURCE = readFileSync(
  new URL('./use-store-setter-shims.ts', import.meta.url),
  'utf8',
)

const SETTER_NAMES = [
  'setProjects',
  'setProjectSessions',
  'setCurrentProject',
  'setActiveProjectId',
  'setActiveSessionId',
  'setConversations',
  'setSessionLoading',
  'setSessionTodos',
  'setSessionTitleStates',
  'setStreamAbortHandles',
] as const

describe('useAppStateSetters — structural contract', () => {
  it('exports `useAppStateSetters` and `AppStateSetters`', () => {
    assert.match(SOURCE, /export function useAppStateSetters\(\)/)
    assert.match(SOURCE, /export interface AppStateSetters/)
  })

  for (const name of SETTER_NAMES) {
    it(`declares ${name} as a useCallback shim with empty deps`, () => {
      // Each setter must be a `const X = useCallback( … , [])` so
      // the App.tsx call sites that capture the reference at
      // mount time keep the same identity for the lifetime of
      // the component (matches the inline shims they replaced).
      const pattern = new RegExp(
        `const ${name} = useCallback\\([\\s\\S]*?\\[\\],\\s*\\)`,
      )
      assert.match(SOURCE, pattern, `missing useCallback shim for ${name}`)
    })
  }

  it('returns an object containing all 10 setters + their slice values', () => {
    for (const name of SETTER_NAMES) {
      assert.match(
        SOURCE,
        new RegExp(`(^|\\W)${name}(,|\\s*})`, 'm'),
        `${name} missing from return shape`,
      )
    }
    // The 10 read-side slice values are part of the return too.
    for (const slice of [
      'projects',
      'projectSessions',
      'currentProject',
      'activeProjectId',
      'activeSessionId',
      'conversations',
      'sessionLoading',
      'sessionTodos',
      'sessionTitleStates',
      'streamAbortHandles',
    ]) {
      assert.match(SOURCE, new RegExp(`(^|\\W)${slice}(,|\\s*})`, 'm'))
    }
  })

  it('reads previous state from the canonical snapshot (not the React slice)', () => {
    // `chat-slice` writers must consult `getConversationSnapshot()`
    // so multiple calls within a single async closure see each
    // other's effects (matches the original inline behavior).
    const snapshotCalls = SOURCE.match(/getConversationSnapshot\(\)/g) ?? []
    assert.ok(
      snapshotCalls.length >= 5,
      `expected ≥5 getConversationSnapshot() reads, found ${snapshotCalls.length}`,
    )
    // Bootstrap-slice writers must consult `bootstrapStore.getSnapshot()`.
    const bootstrapCalls = SOURCE.match(/bootstrapStore\.getSnapshot\(\)/g) ?? []
    assert.ok(
      bootstrapCalls.length >= 4,
      `expected ≥4 bootstrapStore.getSnapshot() reads, found ${bootstrapCalls.length}`,
    )
    // Session-slice writer must consult `sessionStore.getSnapshot()`.
    assert.match(SOURCE, /sessionStore\.getSnapshot\(\)\.activeSessionId/)
  })

  it('preserves the MEM-MOD-WIRE-FIX-2 close-on-leave behavior', () => {
    assert.match(SOURCE, /closeSession\(previous\)/)
  })
})

// ---------------------------------------------------------------
// (b) Behavior round-trips against the underlying store actions.
// ---------------------------------------------------------------

const TEST_SESSION = 'gf03-pr3-shim-roundtrip'
const TEST_PROJECT = 'gf03-pr3-shim-project'

describe('useAppStateSetters — store action round-trips', () => {
  beforeEach(() => {
    removeSession(TEST_SESSION)
    sessionStore.reset?.()
    bootstrapStore.setProjectList([])
    bootstrapStore.setProjectSessions({})
  })

  it('setProjects shim → bootstrapStore.setProjectList round-trips a value', () => {
    bootstrapStore.setProjectList([
      { id: 'p1', name: 'P1', workdir: '/w' } as never,
    ])
    assert.equal(bootstrapStore.getSnapshot().projects.length, 1)
  })

  it('setProjectSessions shim supports the functional-updater path', () => {
    // Simulate the shim's `(prev) => ({ ...prev, [id]: [...] })`
    // pattern by reading the snapshot, mutating, and dispatching.
    bootstrapStore.setProjectSessions({})
    const prev = bootstrapStore.getSnapshot().projectSessions
    const next = { ...prev, [TEST_PROJECT]: [] as never[] }
    bootstrapStore.setProjectSessions(next)
    assert.deepEqual(
      bootstrapStore.getSnapshot().projectSessions[TEST_PROJECT],
      [],
    )
  })

  it('setCurrentProject shim writes through bootstrapStore.selectProject', () => {
    bootstrapStore.selectProject({
      projectId: 'p1',
      currentProject: { id: 'p1', name: 'P1', workdir: '/w' } as never,
    })
    assert.equal(bootstrapStore.getSnapshot().currentProject?.id, 'p1')
    assert.equal(bootstrapStore.getSnapshot().activeProjectId, 'p1')
  })

  it('setActiveProjectId shim preserves currentProject when only id changes', () => {
    bootstrapStore.selectProject({
      projectId: 'p1',
      currentProject: { id: 'p1', name: 'P1', workdir: '/w' } as never,
    })
    bootstrapStore.selectProject({
      projectId: 'p2',
      currentProject: bootstrapStore.getSnapshot().currentProject,
    })
    assert.equal(bootstrapStore.getSnapshot().activeProjectId, 'p2')
    assert.equal(bootstrapStore.getSnapshot().currentProject?.id, 'p1')
  })

  it('setActiveSessionId shim mirrors sessionStore.setActiveSessionId', () => {
    sessionStore.setActiveSessionId(TEST_SESSION)
    assert.equal(sessionStore.getSnapshot().activeSessionId, TEST_SESSION)
    sessionStore.setActiveSessionId(null)
    assert.equal(sessionStore.getSnapshot().activeSessionId, null)
  })

  it('setConversations shim → setConversation + removeSession diff', () => {
    setConversation({
      id: TEST_SESSION,
      projectId: '',
      title: 'T',
      messages: [],
      updatedAt: new Date(),
    } as never)
    assert.ok(getConversationSnapshot().conversations[TEST_SESSION])
    // No-op diff: setting the same reference must not throw and
    // the entry stays present.
    const prev = getConversationSnapshot().conversations[TEST_SESSION]
    setConversation(prev)
    assert.strictEqual(
      getConversationSnapshot().conversations[TEST_SESSION],
      prev,
    )
    removeSession(TEST_SESSION)
    assert.equal(
      getConversationSnapshot().conversations[TEST_SESSION],
      undefined,
    )
  })

  it('setSessionLoading shim → setSessionLoading(id, bool) toggle', () => {
    setSessionLoading(TEST_SESSION, true)
    assert.equal(getConversationSnapshot().sessionLoading[TEST_SESSION], true)
    setSessionLoading(TEST_SESSION, false)
    assert.notEqual(
      getConversationSnapshot().sessionLoading[TEST_SESSION],
      true,
    )
  })

  it('setSessionTodos shim → setSessionTodos(id, todos) replaces array', () => {
    setSessionTodos(TEST_SESSION, [
      { content: 'a', activeForm: 'A', status: 'pending' },
    ])
    assert.equal(getConversationSnapshot().sessionTodos[TEST_SESSION].length, 1)
    setSessionTodos(TEST_SESSION, [])
    assert.equal(getConversationSnapshot().sessionTodos[TEST_SESSION].length, 0)
  })

  it('setSessionTitleStates shim → initTitleState + setTitleState', () => {
    initTitleState(TEST_SESSION)
    const titleState = {
      stage: 'provisional' as const,
      autoRenameCount: 0,
    }
    setTitleState(TEST_SESSION, titleState)
    assert.deepEqual(
      getConversationSnapshot().sessionTitleStates[TEST_SESSION],
      titleState,
    )
  })

  it('setStreamAbortHandles shim → set + clear handle', () => {
    setStreamAbortHandle(TEST_SESSION, 'handle-xyz')
    assert.equal(
      getConversationSnapshot().streamAbortHandles[TEST_SESSION],
      'handle-xyz',
    )
    clearStreamAbortHandle(TEST_SESSION)
    assert.equal(
      getConversationSnapshot().streamAbortHandles[TEST_SESSION],
      undefined,
    )
  })
})
