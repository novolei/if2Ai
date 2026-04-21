// MIG-012 — slash-command domain facade.

import { getApiClient } from './client.ts'

/** Suggest slash-command completions for the given input prefix. */
export async function suggestSlashCommands(input: string, limit = 8): Promise<string[]> {
  return getApiClient().call<string[]>('suggest_slash_commands', { input, limit })
}

/** Execute a slash command in the context of the given session. */
export async function executeSlashCommand(input: string, sessionId: string): Promise<string> {
  return getApiClient().call<string>('execute_slash_command', { input, sessionId })
}

/** Resolve `/skill-name [instruction]` into a full skill-
 * invocation message (containing the SKILL.md content) that
 * can be sent directly to the agent. Returns `null` if the
 * input does not match any installed skill. */
export async function resolveSkillSlash(input: string, cwd?: string): Promise<string | null> {
  return getApiClient().call<string | null>('resolve_skill_slash', {
    input,
    cwd: cwd ?? null,
  })
}
