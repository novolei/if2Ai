#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { basename, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const root = resolve(import.meta.dirname, '..')
const args = new Map()
for (const arg of process.argv.slice(2)) {
  const match = arg.match(/^--([^=]+)=(.*)$/)
  if (match) args.set(match[1], match[2])
  else if (arg === '--dry-run') args.set('dry-run', 'true')
}

const ownerRepo = args.get('repo') || process.env.GITHUB_REPOSITORY || 'novolei/if2Ai'
const channel = args.get('channel') || 'stable'
const outDir = resolve(root, args.get('out-dir') || process.env.RELEASE_OUT_DIR || `${process.env.HOME}/Desktop/if2aiwen_pkg`)
const artifactArg = args.get('artifact')
const updaterArtifactArg = args.get('updater-artifact')
const signatureArg = args.get('signature')
const build = args.get('build')
if (build) {
  run('bash', ['scripts/release-macos.sh', build])
}
const effectivePackageJson = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'))
const effectiveVersion = args.get('version') || effectivePackageJson.version
const effectiveTag = args.get('tag') || `v${effectiveVersion}`
const artifactPath = artifactArg ? resolve(root, artifactArg) : findArtifact(outDir, effectiveVersion)
const updaterArtifactPath = updaterArtifactArg
  ? resolve(root, updaterArtifactArg)
  : findUpdaterArtifact(effectiveVersion)
const signaturePath = signatureArg ? resolve(root, signatureArg) : `${updaterArtifactPath}.sig`
const manifestDir = resolve(root, args.get('manifest-dir') || 'dist/release')
const manifestPath = resolve(manifestDir, 'if2ai-release-manifest.json')
const tauriManifestPath = resolve(manifestDir, 'latest.json')
const dryRun = args.get('dry-run') === 'true'

if (!existsSync(artifactPath)) {
  throw new Error(`release artifact not found: ${artifactPath}`)
}
if (!existsSync(updaterArtifactPath)) {
  throw new Error(`signed updater artifact not found: ${updaterArtifactPath}`)
}
if (!existsSync(signaturePath)) {
  throw new Error(`updater signature not found: ${signaturePath}`)
}

mkdirSync(manifestDir, { recursive: true })

const artifactName = basename(artifactPath)
const updaterArtifactName = basename(updaterArtifactPath)
const updaterSignatureName = basename(signaturePath)
const checksum = sha256File(updaterArtifactPath)
const signature = readFileSync(signaturePath, 'utf8').trim()
const releaseUrl = `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/${encodeURIComponent(artifactName)}`
const updaterUrl = `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/${encodeURIComponent(updaterArtifactName)}`
const updaterSignatureUrl = `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/${encodeURIComponent(updaterSignatureName)}`
const manifestUrl = `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/if2ai-release-manifest.json`
const tauriManifestUrl = `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/latest.json`
const platform =
  process.platform === 'darwin' ? 'darwin' : process.platform === 'win32' ? 'windows' : 'linux'
const arch = process.arch === 'arm64' ? 'aarch64' : process.arch === 'x64' ? 'x86_64' : process.arch
const tauriTarget = `${platform}-${arch}`
const manifest = {
  schema: 'if2ai.release-manifest',
  version: 1,
  channel,
  latest_version: effectiveVersion,
  release_notes_url: `https://github.com/${ownerRepo}/releases/tag/${effectiveTag}`,
  published_at: new Date().toISOString(),
  artifacts: [
    {
      platform,
      arch,
      url: updaterUrl,
      installer_url: releaseUrl,
      signature_url: updaterSignatureUrl,
      checksum_sha256: checksum,
      signature,
    },
  ],
}
const tauriManifest = {
  version: effectiveVersion,
  notes: `If2Ai ${effectiveTag}`,
  pub_date: manifest.published_at,
  release_notes_url: `https://github.com/${ownerRepo}/releases/tag/${effectiveTag}`,
  platforms: {
    [tauriTarget]: {
      signature,
      url: updaterUrl,
    },
  },
}

writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
writeFileSync(tauriManifestPath, `${JSON.stringify(tauriManifest, null, 2)}\n`)
run('node', ['scripts/validate-release-manifest.mjs', manifestPath, channel])

console.log(`manifest: ${manifestPath}`)
console.log(`tauri_manifest: ${tauriManifestPath}`)
console.log(`artifact: ${artifactPath}`)
console.log(`updater_artifact: ${updaterArtifactPath}`)
console.log(`manifest_url: ${manifestUrl}`)
console.log(`tauri_manifest_url: ${tauriManifestUrl}`)

if (dryRun) {
  console.log('dry-run: skipped gh release upload')
  process.exit(0)
}

run('gh', ['auth', 'status'])
const releaseExists = spawnSync('gh', ['release', 'view', effectiveTag, '--repo', ownerRepo], {
  cwd: root,
  encoding: 'utf8',
})
if (releaseExists.status === 0) {
  run('gh', ['release', 'upload', effectiveTag, artifactPath, updaterArtifactPath, signaturePath, manifestPath, tauriManifestPath, '--repo', ownerRepo, '--clobber'])
} else {
  run('gh', [
    'release',
    'create',
    effectiveTag,
    artifactPath,
    updaterArtifactPath,
    signaturePath,
    manifestPath,
    tauriManifestPath,
    '--repo',
    ownerRepo,
    '--title',
    `If2Ai ${effectiveTag}`,
    '--notes',
    `If2Ai ${effectiveTag}\n\nUpdater manifest: ${manifestUrl}\nTauri updater manifest: ${tauriManifestUrl}`,
  ])
}

function findArtifact(dir, artifactVersion) {
  if (!existsSync(dir)) {
    throw new Error(`release out dir not found: ${dir}`)
  }
  const files = readdirSync(dir)
    .filter((file) => file.endsWith('.dmg') || file.endsWith('.pkg') || file.endsWith('.zip'))
    .filter((file) => file.includes(artifactVersion))
    .sort((a, b) => scoreArtifact(b) - scoreArtifact(a))
  if (files.length === 0) {
    throw new Error(`no .dmg/.pkg/.zip artifact for ${artifactVersion} in ${dir}`)
  }
  return resolve(dir, files[0])
}

function findUpdaterArtifact(artifactVersion) {
  const bundleRoots = [
    resolve(root, 'target/release/bundle'),
    resolve(root, 'src-tauri/target/release/bundle'),
  ]
  const candidates = bundleRoots.flatMap((bundleRoot) => collectFiles(bundleRoot))
    .filter((file) => file.endsWith('.app.tar.gz') || file.endsWith('.AppImage') || file.endsWith('.msi.zip') || file.endsWith('.nsis.zip') || file.endsWith('.exe'))
    .filter((file) => file.includes(artifactVersion) || basename(file).includes('If2Ai.app.tar.gz'))
    .filter((file) => existsSync(`${file}.sig`))
    .sort((a, b) => scoreUpdaterArtifact(b) - scoreUpdaterArtifact(a))
  if (candidates.length === 0) {
    throw new Error(`no signed Tauri updater artifact for ${artifactVersion} under ${bundleRoots.join(', ')}`)
  }
  return candidates[0]
}

function collectFiles(dir) {
  if (!existsSync(dir)) return []
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(dir, entry.name)
    return entry.isDirectory() ? collectFiles(path) : [path]
  })
}

function scoreUpdaterArtifact(file) {
  if (file.endsWith('.app.tar.gz')) return 5
  if (file.endsWith('.nsis.zip')) return 4
  if (file.endsWith('.msi.zip')) return 3
  if (file.endsWith('.AppImage')) return 2
  return 1
}

function scoreArtifact(file) {
  if (file.endsWith('.dmg')) return 3
  if (file.endsWith('.pkg')) return 2
  return 1
}

function sha256File(path) {
  const hash = createHash('sha256')
  hash.update(readFileSync(path))
  const size = statSync(path).size
  if (size <= 0) throw new Error(`artifact is empty: ${path}`)
  return hash.digest('hex')
}

function run(command, commandArgs) {
  const result = spawnSync(command, commandArgs, { cwd: root, stdio: 'inherit' })
  if (result.status !== 0) {
    throw new Error(`${command} ${commandArgs.join(' ')} failed`)
  }
}
