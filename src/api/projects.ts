// MIG-012 — project domain facade.
//
// Covers project CRUD, worktree provisioning, workdir bootstrap,
// and the OS-level folder picker the onboarding flow uses.

import type { Project, ProjectMeta, SessionMeta } from '@/lib/tauri'

import { getApiClient } from './client.ts'

export type { Project, ProjectMeta }

/** Create a new project rooted at `workdir`. */
export async function createProject(name: string, workdir: string): Promise<Project> {
  return getApiClient().call<Project>('create_project', { name, workdir })
}

/** List every project known to the backend. */
export async function listProjects(): Promise<ProjectMeta[]> {
  return getApiClient().call<ProjectMeta[]>('list_projects')
}

/** Rename a project. Returns the updated record. */
export async function renameProject(id: string, newName: string): Promise<Project> {
  return getApiClient().call<Project>('rename_project', { id, newName })
}

/** Delete a project by id. */
export async function deleteProject(id: string): Promise<void> {
  return getApiClient().call<void>('delete_project', { id })
}

/** Reveal a project in the OS file manager. */
export async function openProjectInFinder(id: string): Promise<void> {
  return getApiClient().call<void>('open_project_in_finder', { id })
}

/** Provision a permanent worktree for a project; returns its
 * filesystem path. */
export async function createPermanentWorktree(id: string): Promise<string> {
  return getApiClient().call<string>('create_permanent_worktree', { id })
}

/** List the sessions that belong to the given project. */
export async function listProjectSessions(projectId: string): Promise<SessionMeta[]> {
  return getApiClient().call<SessionMeta[]>('list_project_sessions', { projectId })
}

/** Open the native folder-picker dialog. Resolves to `null`
 * when the user cancels. */
export async function pickFolderDialog(): Promise<string | null> {
  return getApiClient().call<string | null>('pick_folder_dialog')
}

/** Ensure the default `~/Documents/workaround` workdir exists
 * and is registered as the "Playground" project. Returns
 * `[workdirPath, projectId]`. */
export async function ensureDefaultWorkdir(): Promise<[string, string]> {
  return getApiClient().call<[string, string]>('ensure_default_workdir')
}
