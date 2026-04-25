import { readFileSync } from 'node:fs'

const manifestPath = process.argv[2] || process.env.MANIFEST_PATH
const expectedChannel = process.argv[3] || process.env.RELEASE_CHANNEL || 'stable'

if (!manifestPath) {
  throw new Error('manifest path is required')
}

const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))

if (manifest.schema !== 'if2ai.release-manifest') {
  throw new Error(`schema mismatch: ${manifest.schema}`)
}
if (manifest.version !== 1) {
  throw new Error(`unsupported manifest version: ${manifest.version}`)
}
if (manifest.channel !== expectedChannel) {
  throw new Error(`channel mismatch: expected=${expectedChannel}, actual=${manifest.channel}`)
}
if (!manifest.latest_version || typeof manifest.latest_version !== 'string') {
  throw new Error('latest_version is required')
}
if (!Array.isArray(manifest.artifacts) || manifest.artifacts.length === 0) {
  throw new Error('at least one release artifact is required')
}

for (const artifact of manifest.artifacts) {
  if (!artifact.platform || !artifact.arch) {
    throw new Error('artifact platform and arch are required')
  }
  if (!/^https:\/\//.test(artifact.url || '')) {
    throw new Error(`artifact url must use https: ${artifact.url}`)
  }
  const hasSignature = typeof artifact.signature === 'string' && artifact.signature.trim().length > 0
  const hasSha256 = typeof artifact.checksum_sha256 === 'string' && /^[a-fA-F0-9]{64}$/.test(artifact.checksum_sha256)
  if (!hasSignature && !hasSha256) {
    throw new Error(`${artifact.platform}/${artifact.arch} requires signature or checksum_sha256`)
  }
  if (expectedChannel === 'stable' && (!hasSignature || artifact.signature.startsWith('sha256:'))) {
    throw new Error(`${artifact.platform}/${artifact.arch} stable release requires a real Tauri updater signature`)
  }
}

console.log(`validated ${manifest.artifacts.length} updater artifact(s) for ${manifest.channel} ${manifest.latest_version}`)
