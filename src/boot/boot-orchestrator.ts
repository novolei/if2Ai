// MIG-013 — boot orchestrator.
//
// Encapsulates the boot sequence that used to live as a 90-line
// `useEffect` at the top of `App.tsx`. Signature is explicit
// about every dependency it needs (no reaching into module
// singletons) so the orchestrator is unit-testable with
// recording mocks.
//
// Sequence (matches the original `App.tsx::boot()`):
//
//   1. Check `getOnboardingState()` with a 3s timeout. If
//      `first_launch` / `onboarding`, transition to the
//      onboarding surface and stop.
//   2. Enforce a minimum 800 ms splash hold so the splash does
//      not flash on fast machines.
//   3. Provision the default workdir + project, load the full
//      project list, preload every project's sessions.
//   4. Pick the default project as `activeProjectId` when
//      available.
//   5. Signal `bootReady(...)` so the AppShell transitions to
//      the main surface.
//
// A fatal failure at step 3 (e.g. `listProjects` throws) calls
// `bootFailed(error)` so the AppShell can surface a recoverable
// dialog instead of hanging on the splash.

import type {
  awaitGatewayReady as AwaitGatewayReadyFn,
  ensureDefaultWorkdir as EnsureDefaultWorkdirFn,
  getOnboardingState as GetOnboardingStateFn,
  listProjects as ListProjectsFn,
  listProjectSessions as ListProjectSessionsFn,
  Project,
  ProjectMeta,
  SessionMeta,
} from '@/api'

import type { BootstrapStore } from '@/state'

/** Dependencies the orchestrator needs. Every callable maps to a
 * `src/api/*` facade function so tests can inject mocks
 * without touching Tauri IPC. */
export interface BootDependencies {
  awaitGatewayReady?: typeof AwaitGatewayReadyFn
  getOnboardingState: typeof GetOnboardingStateFn
  ensureDefaultWorkdir: typeof EnsureDefaultWorkdirFn
  listProjects: typeof ListProjectsFn
  listProjectSessions: typeof ListProjectSessionsFn
  /** Optional knob used by tests to avoid the 800 ms splash-hold
   * delay. Defaults to the production 800 ms value. */
  splashHoldMs?: number
  /** Optional knob used by tests to shrink the onboarding-check
   * timeout. Defaults to 3000 ms. */
  onboardingTimeoutMs?: number
  /** Optional external signal (test-only) used to abort the
   * orchestrator mid-run. */
  signal?: { cancelled: boolean }
}

/** Non-recoverable outcome surfaced back to the caller. Today
 * the caller only uses it for test assertions; production code
 * reads the result from the store. */
export type BootOutcome = 'onboarding' | 'ready' | 'error'

/** Run one boot sequence against the provided store. */
export async function runBootSequence(
  store: BootstrapStore,
  deps: BootDependencies,
): Promise<BootOutcome> {
  const {
    awaitGatewayReady,
    getOnboardingState,
    ensureDefaultWorkdir,
    listProjects,
    listProjectSessions,
    splashHoldMs = 800,
    onboardingTimeoutMs = 3000,
    signal,
  } = deps

  const isCancelled = () => signal?.cancelled === true

  const onboardingCheck = (async () => {
    try {
      const raw = await Promise.race([
        getOnboardingState(),
        new Promise<never>((_, reject) =>
          setTimeout(
            () => reject(new Error('onboarding_get_state timeout')),
            onboardingTimeoutMs,
          ),
        ),
      ])
      const obj = raw as Record<string, unknown>
      const tag = obj.state as string | undefined
      return tag === 'first_launch' || tag === 'onboarding'
    } catch (err) {
      console.warn('[boot] onboarding check failed, defaulting to no onboarding:', err)
      return false
    }
  })()

  const splashTimer = new Promise<void>((resolve) => {
    setTimeout(resolve, splashHoldMs)
  })

  const [isOnboarding] = await Promise.all([onboardingCheck, splashTimer])

  if (isCancelled()) return 'ready'

  if (isOnboarding) {
    store.enterOnboarding()
    return 'onboarding'
  }

  try {
    if (awaitGatewayReady) {
      await awaitGatewayReady()
    }

    let defaultProjectId: string | null = null
    try {
      const [, projId] = await ensureDefaultWorkdir()
      defaultProjectId = projId
    } catch {
      // Non-fatal — continue without a default project.
    }

    const projectList = await listProjects()
    const sessionsMap: Record<string, SessionMeta[]> = {}
    for (const project of projectList) {
      sessionsMap[project.id] = await listProjectSessions(project.id)
    }

    const defaultProject = defaultProjectId
      ? projectList.find((item) => item.id === defaultProjectId) ?? null
      : null
    const currentProject: Project | null = defaultProject
      ? {
          id: defaultProject.id,
          name: defaultProject.name,
          workdir: defaultProject.workdir,
          created_at: defaultProject.created_at,
          updated_at: '',
        }
      : null

    if (isCancelled()) return 'ready'

    store.bootReady({
      projects: projectList,
      projectSessions: sessionsMap,
      activeProjectId: defaultProject?.id ?? null,
      currentProject,
    })
    return 'ready'
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    if (isCancelled()) return 'error'
    store.bootFailed(`Failed to load projects during boot: ${message}`)
    console.error('Failed to load projects during boot:', err)
    return 'error'
  }
}

// Re-export the types consumed by AppShell so the boot module
// stays a single import root.
export type { ProjectMeta, SessionMeta, Project }
