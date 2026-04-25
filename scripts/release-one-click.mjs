#!/usr/bin/env node
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { basename, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const root = resolve(import.meta.dirname, '..')
const versionFiles = ['package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml']
const args = parseArgs(process.argv.slice(2))
const dryRun = args.flags.has('dry-run')
const skipBuild = args.flags.has('skip-build')
const skipChecks = args.flags.has('skip-checks')
const localUpload = args.flags.has('local-upload')
const noDispatch = args.flags.has('no-dispatch')
const allowVersionDirty = args.flags.has('allow-version-dirty')
const bump = args.values.get('bump') || args.positionals[0] || 'patch'
const explicitVersion = args.values.get('version')
const repo = args.values.get('repo') || process.env.GITHUB_REPOSITORY || 'novolei/if2Ai'
const channel = args.values.get('channel') || 'stable'
const remote = args.values.get('remote') || 'origin'
const targets = args.values.get('targets') || 'arm64'
const currentBranch = gitOutput(['branch', '--show-current'])
const currentVersion = readVersion()
const nextVersion = explicitVersion || bumpVersion(currentVersion, bump)
const tag = args.values.get('tag') || `v${nextVersion}`

assertSemver(nextVersion)
assertTargets(targets)
assertCleanVersionFiles()
assertTagIsAvailable(tag)

console.log(`release: ${currentVersion} -> ${nextVersion}`)
console.log(`tag: ${tag}`)
console.log(`repo: ${repo}`)
console.log(`branch: ${currentBranch}`)
console.log(`ci_targets: ${targets}`)

if (dryRun) {
  console.log('dry-run: no files changed, no build, no commit, no tag, no push')
  process.exit(0)
}

bumpFiles(nextVersion)

if (!skipChecks) {
  run('npm', ['run', 'check:version'])
  run('node', ['--check', 'scripts/publish-github-release.mjs'])
}

let artifacts = null
if (!skipBuild) {
  const buildEnv = signedBuildEnv()
  run('npm', ['run', 'build'], { env: buildEnv })
  artifacts = findReleaseArtifacts(nextVersion)
  run('node', [
    'scripts/publish-github-release.mjs',
    '--dry-run',
    `--repo=${repo}`,
    `--channel=${channel}`,
    `--version=${nextVersion}`,
    `--tag=${tag}`,
    `--artifact=${artifacts.installer}`,
    `--updater-artifact=${artifacts.updater}`,
    `--signature=${artifacts.signature}`,
  ])
}

run('git', ['add', ...versionFiles])
run('git', ['commit', '-m', `chore(release): ${tag}`])
run('git', ['tag', '-a', tag, '-m', `Release If2Ai ${tag}`])
run('git', ['push', remote, currentBranch])
run('git', ['push', remote, tag])

if (localUpload) {
  const uploadArtifacts = artifacts || findReleaseArtifacts(nextVersion)
  run('node', [
    'scripts/publish-github-release.mjs',
    `--repo=${repo}`,
    `--channel=${channel}`,
    `--version=${nextVersion}`,
    `--tag=${tag}`,
    `--artifact=${uploadArtifacts.installer}`,
    `--updater-artifact=${uploadArtifacts.updater}`,
    `--signature=${uploadArtifacts.signature}`,
  ])
} else if (!noDispatch) {
  run('gh', [
    'workflow',
    'run',
    'release.yml',
    '--repo',
    repo,
    '--ref',
    tag,
    '-f',
    `release_targets=${targets}`,
    '-f',
    `channel=${channel}`,
  ])
  console.log(`dispatched release workflow for ${tag} (${targets})`)
  console.log(`workflow: https://github.com/${repo}/actions/workflows/release.yml`)
} else {
  console.log(`pushed ${tag}; skipped release workflow dispatch because --no-dispatch was set`)
  console.log(`workflow: https://github.com/${repo}/actions/workflows/release.yml`)
}

function parseArgs(argv) {
  const values = new Map()
  const flags = new Set()
  const positionals = []
  for (const arg of argv) {
    if (arg.startsWith('--')) {
      const match = arg.match(/^--([^=]+)=(.*)$/)
      if (match) values.set(match[1], match[2])
      else flags.add(arg.slice(2))
    } else {
      positionals.push(arg)
    }
  }
  return { values, flags, positionals }
}

function readVersion() {
  const packageJson = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'))
  return packageJson.version
}

function bumpVersion(version, releaseType) {
  assertSemver(version)
  const [major, minor, patch] = version.split('.').map(Number)
  if (releaseType === 'major') return `${major + 1}.0.0`
  if (releaseType === 'minor') return `${major}.${minor + 1}.0`
  if (releaseType === 'patch') return `${major}.${minor}.${patch + 1}`
  assertSemver(releaseType)
  return releaseType
}

function assertSemver(version) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`expected semver x.y.z, got: ${version}`)
  }
}

function assertTargets(value) {
  if (!['arm64', 'universal'].includes(value)) {
    throw new Error(`expected --targets=arm64 or --targets=universal, got: ${value}`)
  }
}

function assertCleanVersionFiles() {
  const dirty = gitOutput(['status', '--porcelain', '--', ...versionFiles])
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
  if (dirty.length > 0 && !allowVersionDirty && !dryRun) {
    throw new Error(
      `version files are dirty; commit/stash them first or pass --allow-version-dirty:\n${dirty.join('\n')}`,
    )
  }
}

function assertTagIsAvailable(releaseTag) {
  const local = spawnSync('git', ['rev-parse', '-q', '--verify', `refs/tags/${releaseTag}`], {
    cwd: root,
    encoding: 'utf8',
  })
  if (local.status === 0) throw new Error(`local tag already exists: ${releaseTag}`)

  const remoteTag = spawnSync('git', ['ls-remote', '--tags', remote, releaseTag], {
    cwd: root,
    encoding: 'utf8',
  })
  if (remoteTag.status !== 0) {
    throw new Error(`failed to check remote tag ${releaseTag}: ${remoteTag.stderr.trim()}`)
  }
  if (remoteTag.stdout.trim()) throw new Error(`remote tag already exists: ${releaseTag}`)
}

function bumpFiles(version) {
  const packagePath = resolve(root, 'package.json')
  const packageJson = JSON.parse(readFileSync(packagePath, 'utf8'))
  packageJson.version = version
  writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 4)}\n`)

  const tauriPath = resolve(root, 'src-tauri/tauri.conf.json')
  const tauriConf = JSON.parse(readFileSync(tauriPath, 'utf8'))
  tauriConf.version = version
  writeFileSync(tauriPath, `${JSON.stringify(tauriConf, null, 4)}\n`)

  const cargoPath = resolve(root, 'src-tauri/Cargo.toml')
  const cargoToml = readFileSync(cargoPath, 'utf8')
  const updatedCargoToml = cargoToml.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`)
  if (updatedCargoToml === cargoToml) throw new Error('failed to update src-tauri/Cargo.toml version')
  writeFileSync(cargoPath, updatedCargoToml)
}

function signedBuildEnv() {
  const env = { ...process.env }
  const keyPath = resolve(process.env.HOME || '', '.tauri/if2ai-updater.key')
  const pubkeyPath = `${keyPath}.pub`

  if (!env.TAURI_SIGNING_PRIVATE_KEY && existsSync(keyPath)) {
    env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyPath, 'utf8')
  }
  if (!Object.prototype.hasOwnProperty.call(env, 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD')) {
    env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
  }
  if (!env.IF2AI_UPDATER_PUBKEY && existsSync(pubkeyPath)) {
    env.IF2AI_UPDATER_PUBKEY = readFileSync(pubkeyPath, 'utf8').trim()
  }
  if (!env.TAURI_SIGNING_PRIVATE_KEY) {
    throw new Error('missing TAURI_SIGNING_PRIVATE_KEY and ~/.tauri/if2ai-updater.key')
  }
  if (!env.IF2AI_UPDATER_PUBKEY) {
    throw new Error('missing IF2AI_UPDATER_PUBKEY and ~/.tauri/if2ai-updater.key.pub')
  }
  return env
}

function findReleaseArtifacts(version) {
  const bundleRoots = [
    resolve(root, 'target/release/bundle'),
    resolve(root, 'src-tauri/target/release/bundle'),
  ]
  const files = bundleRoots.flatMap((dir) => collectFiles(dir))
  const installer = newest(
    files.filter((file) => ['.dmg', '.pkg', '.zip'].some((suffix) => file.endsWith(suffix)))
      .filter((file) => basename(file).includes(version))
      .filter((file) => !file.endsWith('.app.tar.gz')),
  )
  const updater = newest(
    files.filter((file) => file.endsWith('.app.tar.gz') && existsSync(`${file}.sig`)),
  )
  if (!installer) throw new Error(`no installer artifact found for ${version} under ${bundleRoots.join(', ')}`)
  if (!updater) throw new Error(`no signed updater artifact found under ${bundleRoots.join(', ')}`)
  return {
    installer,
    updater,
    signature: `${updater}.sig`,
  }
}

function collectFiles(dir) {
  if (!existsSync(dir)) return []
  const entries = spawnSync('find', [dir, '-type', 'f'], { cwd: root, encoding: 'utf8' })
  if (entries.status !== 0) return []
  return entries.stdout.split('\n').filter(Boolean)
}

function newest(files) {
  return files
    .filter((file) => existsSync(file) && statSync(file).size > 0)
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0]
}

function gitOutput(commandArgs) {
  const result = spawnSync('git', commandArgs, { cwd: root, encoding: 'utf8' })
  if (result.status !== 0) {
    throw new Error(`git ${commandArgs.join(' ')} failed: ${result.stderr.trim()}`)
  }
  return result.stdout.trim()
}

function run(command, commandArgs, options = {}) {
  const result = spawnSync(command, commandArgs, {
    cwd: root,
    stdio: 'inherit',
    env: options.env || process.env,
  })
  if (result.status !== 0) {
    throw new Error(`${command} ${commandArgs.join(' ')} failed`)
  }
}
