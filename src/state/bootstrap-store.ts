// MIG-013 — bootstrap store.
//
// Canonical home of the "how is the app booting?" state slice
// extracted out of `App.tsx`. Lives next to (but intentionally
// separate from) the runtime-projection store so the two can
// evolve independently — bootstrap-store holds pre-main-shell
// state (splash / onboarding / initial project list / startup
// errors); runtime-projection-store holds live backend
// projection events.
//
// Mirrors the `useSyncExternalStore`-compatible shape already
// established by `src/runtime-projection/runtime-projection-store.ts`:
// tests create a fresh store via [`createBootstrapStore`]; the
// production singleton is [`bootstrapStore`].
//
// Explicitly NOT a global catch-all (pack guardrail 2): the
// store only holds the minimum state required by the boot
// orchestrator + AppShell to pick the correct surface. Chat /
// session / settings state lands in MIG-014+ stores.

import type { Project, ProjectMeta, SessionMeta } from '@/api'

/** High-level boot phase. Mirrors the three BootShell surfaces
 * plus an explicit `error` terminal state so the AppShell can
 * render a recoverable failure UI instead of a blank screen. */
export type BootPhase = 'splash' | 'onboarding' | 'main' | 'error'

/** Immutable bootstrap state snapshot. */
export interface BootstrapState {
  /** Which top-level surface the AppShell should render. */
  phase: BootPhase
  /** Non-null iff `phase === 'error'`. Human-readable; the
   * AppShell surfaces it in a recoverable dialog. */
  startupError: string | null
  /** Project list returned from `listProjects()` during boot.
   * Empty while `phase === 'splash' | 'onboarding'`. */
  projects: ProjectMeta[]
  /** `projectId -> sessions[]` map populated during boot. */
  projectSessions: Record<string, SessionMeta[]>
  /** Project selected when the main shell first mounts. `null`
   * while boot is still in flight or no default could be
   * provisioned. */
  activeProjectId: string | null
  /** Full project record for [`activeProjectId`] when available. */
  currentProject: Project | null
}

export const INITIAL_BOOTSTRAP_STATE: BootstrapState = Object.freeze({
  phase: 'splash' as const,
  startupError: null,
  projects: [] as ProjectMeta[],
  projectSessions: {} as Record<string, SessionMeta[]>,
  activeProjectId: null,
  currentProject: null,
})

export type BootstrapListener = () => void

export interface BootstrapStore {
  getSnapshot(): BootstrapState
  subscribe(listener: BootstrapListener): () => void

  // ── actions (explicit rather than a generic setState so the
  // store cannot devolve into a global catch-all — pack §8
  // guardrail 2). ──
  /** Boot orchestrator signals that the onboarding state check
   * finished and the user needs to go through onboarding. */
  enterOnboarding(): void
  /** Boot orchestrator finished provisioning default workdir +
   * project list; transition to the main shell with the
   * provided initial state. */
  bootReady(input: {
    projects: ProjectMeta[]
    projectSessions: Record<string, SessionMeta[]>
    activeProjectId: string | null
    currentProject: Project | null
  }): void
  /** Boot orchestrator hit a fatal error. `phase` becomes
   * `'error'`; AppShell renders the recoverable UI. */
  bootFailed(error: string): void
  /** Onboarding flow completed — re-enter splash so the boot
   * orchestrator reruns. */
  onboardingComplete(): void
  /** Active project selection changed at runtime (e.g. user
   * picked a different project in the rail). */
  selectProject(input: { projectId: string | null; currentProject: Project | null }): void
  /** Refresh the project list / per-project sessions after a
   * create / delete / rename. */
  setProjectList(projects: ProjectMeta[]): void
  setProjectSessions(projectSessions: Record<string, SessionMeta[]>): void
}

/** Create a fresh, isolated store instance. Tests call this
 * directly; production code uses the module-level
 * [`bootstrapStore`] singleton below. */
export function createBootstrapStore(): BootstrapStore {
  let state: BootstrapState = INITIAL_BOOTSTRAP_STATE
  const listeners = new Set<BootstrapListener>()

  const notify = () => {
    listeners.forEach((l) => l())
  }

  const replace = (next: BootstrapState) => {
    if (next === state) return
    state = next
    notify()
  }

  return {
    getSnapshot: () => state,
    subscribe: (listener) => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },

    enterOnboarding: () => {
      replace({ ...state, phase: 'onboarding', startupError: null })
    },

    bootReady: ({ projects, projectSessions, activeProjectId, currentProject }) => {
      replace({
        phase: 'main',
        startupError: null,
        projects,
        projectSessions,
        activeProjectId,
        currentProject,
      })
    },

    bootFailed: (error: string) => {
      replace({ ...state, phase: 'error', startupError: error })
    },

    onboardingComplete: () => {
      replace({ ...INITIAL_BOOTSTRAP_STATE, phase: 'splash' })
    },

    selectProject: ({ projectId, currentProject }) => {
      replace({ ...state, activeProjectId: projectId, currentProject })
    },

    setProjectList: (projects) => {
      replace({ ...state, projects })
    },

    setProjectSessions: (projectSessions) => {
      replace({ ...state, projectSessions })
    },
  }
}

/** Process-wide bootstrap store singleton. Mirrors the
 * `runtimeProjectionStore` export convention. */
export const bootstrapStore: BootstrapStore = createBootstrapStore()
