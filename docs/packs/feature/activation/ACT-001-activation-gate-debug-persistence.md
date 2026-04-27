# ACT-001: Activation Gate Debug Persistence

## Status
- State: active

## Goal
Debug rebuilds should not look randomly deactivated without a clear reason.
The activation gate must keep production license truth strict, but expose enough
diagnostics to explain when `~/.if2ai/activation/license.json` is missing.

## Spec (verifiable)
- Missing `license.json` still maps to `NeedsActivation` by default.
- Debug builds can explicitly bypass the gate with
  `IF2AI_DEV_BYPASS_ACTIVATION=1`.
- The bypass is not implicit: onboarding completion alone must not unlock the
  main shell.
- License load/save/clear logs include the cache path and non-secret license id.
- Activation snapshot message explains the debug bypass when it is active.

## Files (scope)
- `docs/design-docs/activation/ACT-001-activation-gate-debug-persistence.md`
- `src-tauri/src/modules/application/activation_service.rs`
- `src-tauri/src/modules/application/activation/http_client.rs` (test-only env lock)
- `src-tauri/src/modules/application/activation/license_store.rs`

## Reads
- `docs/packs/CHARTER.md`
- `src-tauri/src/modules/application/activation_service.rs`
- `src-tauri/src/modules/application/license_lifecycle_service.rs`
- `src-tauri/src/modules/application/activation/license_store.rs`

## Contract
- Do not add a second activation truth source.
- Do not restore production fallback from onboarding state.
- Do not change IPC command names, runtime event names, or sqlite schema.
- Do not log refresh tokens, signed payloads, or invite codes.

## Out of Scope
- Remote activation server changes.
- Keychain migration.
- Activation settings UI.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml activation -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- Verify commands pass or any unrelated existing blocker is documented.
