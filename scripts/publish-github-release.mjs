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
const artifactSetArg = args.get('artifact-set')
const build = args.get('build')
if (build) {
  run('bash', ['scripts/release-macos.sh', build])
}
const effectivePackageJson = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'))
const effectiveVersion = args.get('version') || effectivePackageJson.version
const effectiveTag = args.get('tag') || `v${effectiveVersion}`
const releaseArtifacts = artifactSetArg
  ? readArtifactSet(artifactSetArg)
  : [
      inferSingleArtifact({
        artifactPath: artifactArg ? resolve(root, artifactArg) : findArtifact(outDir, effectiveVersion),
        updaterArtifactPath: updaterArtifactArg
          ? resolve(root, updaterArtifactArg)
          : findUpdaterArtifact(effectiveVersion),
        signaturePath: signatureArg,
      }),
    ]
const manifestDir = resolve(root, args.get('manifest-dir') || 'dist/release')
const manifestPath = resolve(manifestDir, 'if2ai-release-manifest.json')
const tauriManifestPath = resolve(manifestDir, 'latest.json')
const dryRun = args.get('dry-run') === 'true'

for (const artifact of releaseArtifacts) {
  if (!existsSync(artifact.installer_path)) {
    throw new Error(`release artifact not found: ${artifact.installer_path}`)
  }
  if (!existsSync(artifact.updater_path)) {
    throw new Error(`signed updater artifact not found: ${artifact.updater_path}`)
  }
  if (!existsSync(artifact.signature_path)) {
    throw new Error(`updater signature not found: ${artifact.signature_path}`)
  }
}

mkdirSync(manifestDir, { recursive: true })

const manifestUrl = `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/if2ai-release-manifest.json`
const tauriManifestUrl = `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/latest.json`
const publishedAt = new Date().toISOString()
const manifestArtifacts = releaseArtifacts.map((artifact) => {
  const installerName = basename(artifact.installer_path)
  const updaterName = basename(artifact.updater_path)
  const signatureName = basename(artifact.signature_path)
  return {
    platform: artifact.platform,
    arch: artifact.arch,
    url: `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/${encodeURIComponent(updaterName)}`,
    installer_url: `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/${encodeURIComponent(installerName)}`,
    signature_url: `https://github.com/${ownerRepo}/releases/download/${effectiveTag}/${encodeURIComponent(signatureName)}`,
    checksum_sha256: sha256File(artifact.updater_path),
    signature: readFileSync(artifact.signature_path, 'utf8').trim(),
  }
})
const tauriPlatforms = Object.fromEntries(
  releaseArtifacts.map((artifact, index) => [
    artifact.tauri_target || `${artifact.platform}-${artifact.arch}`,
    {
      signature: manifestArtifacts[index].signature,
      url: manifestArtifacts[index].url,
    },
  ]),
)
const manifest = {
  schema: 'if2ai.release-manifest',
  version: 1,
  channel,
  latest_version: effectiveVersion,
  release_notes_url: `https://github.com/${ownerRepo}/releases/tag/${effectiveTag}`,
  published_at: publishedAt,
  artifacts: manifestArtifacts,
}
const tauriManifest = {
  version: effectiveVersion,
  notes: `If2Ai ${effectiveTag}`,
  pub_date: manifest.published_at,
  release_notes_url: `https://github.com/${ownerRepo}/releases/tag/${effectiveTag}`,
  platforms: tauriPlatforms,
}

writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
writeFileSync(tauriManifestPath, `${JSON.stringify(tauriManifest, null, 2)}\n`)
run('node', ['scripts/validate-release-manifest.mjs', manifestPath, channel])

console.log(`manifest: ${manifestPath}`)
console.log(`tauri_manifest: ${tauriManifestPath}`)
for (const artifact of releaseArtifacts) {
  console.log(`artifact[${artifact.tauri_target || `${artifact.platform}-${artifact.arch}`}]: ${artifact.installer_path}`)
  console.log(`updater_artifact[${artifact.tauri_target || `${artifact.platform}-${artifact.arch}`}]: ${artifact.updater_path}`)
}
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
  run('gh', ['release', 'upload', effectiveTag, ...uploadPaths(), manifestPath, tauriManifestPath, '--repo', ownerRepo, '--clobber'])
} else {
  run('gh', [
    'release',
    'create',
    effectiveTag,
    ...uploadPaths(),
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

function inferSingleArtifact({ artifactPath, updaterArtifactPath, signaturePath }) {
  const platform =
    process.platform === 'darwin' ? 'darwin' : process.platform === 'win32' ? 'windows' : 'linux'
  const arch = process.arch === 'arm64' ? 'aarch64' : process.arch === 'x64' ? 'x86_64' : process.arch
  return {
    platform,
    arch,
    tauri_target: `${platform}-${arch}`,
    installer_path: artifactPath,
    updater_path: updaterArtifactPath,
    signature_path: signaturePath ? resolve(root, signaturePath) : `${updaterArtifactPath}.sig`,
  }
}

function readArtifactSet(path) {
  const artifactSetPath = resolve(root, path)
  const artifactSet = JSON.parse(readFileSync(artifactSetPath, 'utf8'))
  if (!Array.isArray(artifactSet.artifacts) || artifactSet.artifacts.length === 0) {
    throw new Error(`artifact set requires non-empty artifacts: ${artifactSetPath}`)
  }
  return artifactSet.artifacts.map((artifact) => {
    const platform = artifact.platform
    const arch = artifact.arch
    if (!platform || !arch) {
      throw new Error(`artifact set entry requires platform and arch: ${JSON.stringify(artifact)}`)
    }
    const updaterPath = resolve(root, artifact.updater_path || artifact.updater)
    return {
      platform,
      arch,
      tauri_target: artifact.tauri_target || `${platform}-${arch}`,
      installer_path: resolve(root, artifact.installer_path || artifact.installer),
      updater_path: updaterPath,
      signature_path: resolve(root, artifact.signature_path || artifact.signature || `${updaterPath}.sig`),
    }
  })
}

function uploadPaths() {
  return releaseArtifacts.flatMap((artifact) => [
    artifact.installer_path,
    artifact.updater_path,
    artifact.signature_path,
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
