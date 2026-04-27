import type { JiaochangAudioPluginManifest } from './plugin-source-adapter.ts'

export interface JiaochangPluginManifestDraft {
  pluginId: string
  name: string
  version: string
  sourceUrl: string
  signature: string
  enabled: boolean
  allowedHosts: string
  resolverTemplate: string
}

export const DEFAULT_JIACHANG_PLUGIN_MANIFEST_DRAFT: JiaochangPluginManifestDraft = {
  pluginId: '',
  name: '',
  version: '',
  sourceUrl: '',
  signature: '',
  enabled: true,
  allowedHosts: '',
  resolverTemplate: '',
}

export function draftToPluginManifest(draft: JiaochangPluginManifestDraft): JiaochangAudioPluginManifest {
  return {
    plugin_id: draft.pluginId.trim(),
    name: draft.name.trim(),
    version: draft.version.trim() || undefined,
    source_url: draft.sourceUrl.trim() || undefined,
    signature: draft.signature.trim() || undefined,
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
    sourceUrl: manifest.source_url ?? '',
    signature: manifest.signature ?? '',
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
  if (draft.sourceUrl.trim() && !/^(https?:\/\/|local:\/\/|file:\/\/)/.test(draft.sourceUrl.trim())) {
    return 'source_url must be http(s), local://, or file://'
  }
  if (draft.signature.trim() && draft.signature.trim().length < 8) return 'signature is too short'
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

export function getPluginTrustAssessment(manifest: JiaochangAudioPluginManifest): {
  level: 'signed' | 'scoped' | 'unverified'
  label: string
  description: string
} {
  if (manifest.compatibility === 'lx_ceru_js') {
    return {
      level: 'unverified',
      label: 'quarantine',
      description: 'LX/Ceru JS plugin resolves inside a Rust-side worker; verify source/signature before enabling.',
    }
  }
  if (manifest.signature) {
    return {
      level: 'signed',
      label: 'signed',
      description: 'Manifest declares a signature. Verify issuer before enabling in production.',
    }
  }
  if (manifest.source_url && manifest.allowed_hosts.length > 0) {
    return {
      level: 'scoped',
      label: 'scoped',
      description: 'Manifest has a source URL and network host allowlist, but no signature.',
    }
  }
  return {
    level: 'unverified',
    label: 'unverified',
    description: 'No signature or source URL. Keep disabled unless you trust the source.',
  }
}
