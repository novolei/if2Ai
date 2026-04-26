import type { JiaochangAudioPluginManifest } from './plugin-source-adapter.ts'

export interface JiaochangPluginManifestDraft {
  pluginId: string
  name: string
  version: string
  enabled: boolean
  allowedHosts: string
  resolverTemplate: string
}

export const DEFAULT_JIACHANG_PLUGIN_MANIFEST_DRAFT: JiaochangPluginManifestDraft = {
  pluginId: '',
  name: '',
  version: '',
  enabled: true,
  allowedHosts: '',
  resolverTemplate: '',
}

export function draftToPluginManifest(draft: JiaochangPluginManifestDraft): JiaochangAudioPluginManifest {
  return {
    plugin_id: draft.pluginId.trim(),
    name: draft.name.trim(),
    version: draft.version.trim() || undefined,
    enabled: draft.enabled,
    allowed_hosts: parseAllowedHosts(draft.allowedHosts),
    resolver_template: draft.resolverTemplate.trim(),
  }
}

export function pluginManifestToDraft(manifest: JiaochangAudioPluginManifest): JiaochangPluginManifestDraft {
  return {
    pluginId: manifest.plugin_id,
    name: manifest.name,
    version: manifest.version ?? '',
    enabled: manifest.enabled,
    allowedHosts: manifest.allowed_hosts.join(', '),
    resolverTemplate: manifest.resolver_template,
  }
}

export function validatePluginManifestDraft(draft: JiaochangPluginManifestDraft): string | null {
  if (!draft.pluginId.trim()) return 'plugin_id is required'
  if (!/^[A-Za-z0-9:_.-]+$/.test(draft.pluginId.trim())) return 'plugin_id contains unsupported characters'
  if (!draft.name.trim()) return 'name is required'
  if (parseAllowedHosts(draft.allowedHosts).length === 0) return 'allowed_hosts must include at least one host'
  if (!draft.resolverTemplate.trim()) return 'resolver_template is required'
  return null
}

export function parseAllowedHosts(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean)
    .filter((item, index, all) => all.indexOf(item) === index)
}
