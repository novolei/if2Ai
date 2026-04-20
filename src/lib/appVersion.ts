// App version — injected at build time by Vite (`define` in vite.config.ts).
//
// Single source of truth: `package.json` `version`. The release script
// (`scripts/release-macos.sh`) keeps `package.json`, `src-tauri/tauri.conf.json`
// and `src-tauri/Cargo.toml` in lockstep, so this value matches the binary
// `CFBundleShortVersionString` shipped in the .app bundle.
//
// Why not Tauri's `getVersion()`? It's async and only works inside the Tauri
// runtime (no-op in pure browser dev). A compile-time constant is sync,
// works in both contexts, and has zero runtime cost.

export const APP_VERSION: string =
  typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '0.0.0-dev'

export const APP_NAME: string =
  typeof __APP_NAME__ !== 'undefined' ? __APP_NAME__ : 'if2ai'

/** Format e.g. "v0.2.0" — used in About page and watermark. */
export const APP_VERSION_LABEL = `v${APP_VERSION}`
