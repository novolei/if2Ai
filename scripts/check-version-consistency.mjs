import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const packageJson = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'))
const tauriConf = JSON.parse(readFileSync(resolve(root, 'src-tauri/tauri.conf.json'), 'utf8'))
const cargoToml = readFileSync(resolve(root, 'src-tauri/Cargo.toml'), 'utf8')
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1]

const versions = {
  'package.json': packageJson.version,
  'src-tauri/tauri.conf.json': tauriConf.version,
  'src-tauri/Cargo.toml': cargoVersion,
}

const unique = new Set(Object.values(versions))
if (unique.size !== 1) {
  console.error('Version mismatch:')
  for (const [file, version] of Object.entries(versions)) {
    console.error(`  ${file}: ${version ?? '(missing)'}`)
  }
  process.exit(1)
}

console.log(`Version OK: ${packageJson.version}`)
