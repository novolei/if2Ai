import assert from 'node:assert/strict'
import { describe, it } from 'node:test'

import { diagnoseMemoryScopeVisibility } from './memoryScopeDiagnostics.ts'

const context = {
  activeProjectId: 'project-a',
  activeSessionId: 'session-a',
}

describe('memoryScopeDiagnostics', () => {
  it('marks global memory as canonical recall visible', () => {
    const diagnostic = diagnoseMemoryScopeVisibility(
      { session_id: null, project_id: null },
      context,
    )
    assert.equal(diagnostic.kind, 'global')
    assert.equal(diagnostic.isCanonicalRecallVisible, true)
  })

  it('marks current session memory as canonical recall visible', () => {
    const diagnostic = diagnoseMemoryScopeVisibility(
      { session_id: 'session-a', project_id: 'project-a' },
      context,
    )
    assert.equal(diagnostic.kind, 'current_session')
    assert.equal(diagnostic.isCanonicalRecallVisible, true)
  })

  it('marks current project memory as canonical recall visible', () => {
    const diagnostic = diagnoseMemoryScopeVisibility(
      { session_id: null, project_id: 'project-a' },
      context,
    )
    assert.equal(diagnostic.kind, 'current_project')
    assert.equal(diagnostic.isCanonicalRecallVisible, true)
  })

  it('marks same-project older-session memory as bounded fallback only', () => {
    const diagnostic = diagnoseMemoryScopeVisibility(
      { session_id: 'session-b', project_id: 'project-a' },
      context,
    )
    assert.equal(diagnostic.kind, 'same_project_fallback')
    assert.equal(diagnostic.isCanonicalRecallVisible, false)
    assert.equal(diagnostic.isFallbackCandidate, true)
  })

  it('marks unprojected older-session memory as browser-only', () => {
    const diagnostic = diagnoseMemoryScopeVisibility(
      { session_id: 'session-b', project_id: null },
      context,
    )
    assert.equal(diagnostic.kind, 'browser_only_unprojected_session')
    assert.equal(diagnostic.isCanonicalRecallVisible, false)
    assert.equal(diagnostic.isFallbackCandidate, false)
  })

  it('marks other-project memory as browser-only', () => {
    const diagnostic = diagnoseMemoryScopeVisibility(
      { session_id: null, project_id: 'project-b' },
      context,
    )
    assert.equal(diagnostic.kind, 'browser_only_other_project')
    assert.equal(diagnostic.isCanonicalRecallVisible, false)
  })
})
