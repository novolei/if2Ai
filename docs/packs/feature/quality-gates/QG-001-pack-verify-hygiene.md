# QG-001: Pack Verify Hygiene

## Status
- State: active

## Goal
Remove known out-of-scope clippy blockers that make otherwise completed Packs
fail `./scripts/pack verify`.

## Spec (verifiable)
- `jiaochang_audio` clippy warnings are fixed without behavior changes.
- `runtime/projection` test no longer leaves an unused variable.
- `smart_browser/policy` uses the efficient slice membership helper.
- `runtime/recoverability` bool assertion uses idiomatic `assert!`.
- No feature behavior, IPC names, runtime events, or schema are changed.

## Files (scope)
- `docs/design-docs/quality-gates/QG-001-pack-verify-hygiene.md`
- `src-tauri/src/commands/agent/mod.rs`
- `src-tauri/src/modules/jiaochang_audio/mod.rs`
- `src-tauri/src/modules/runtime/projection.rs`
- `src-tauri/src/modules/smart_browser/policy.rs`
- `src-tauri/src/modules/runtime/recoverability.rs`
- `src-tauri/src/modules/provider/capabilities.rs`

## Reads
- `docs/packs/CHARTER.md`
- Recent `./scripts/pack verify` output for ACT-001/AWL-008.

## Contract
- Mechanical lint cleanup only.
- Do not expand MCP, AWL, activation, or memory behavior in this Pack.
- Do not silence clippy with `allow` attributes.

## Verify
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- `cargo test --manifest-path src-tauri/Cargo.toml activation -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml memory_recall -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- Verify commands pass or any unrelated external blocker is documented.
