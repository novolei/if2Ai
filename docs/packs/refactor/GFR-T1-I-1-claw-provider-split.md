# GFR-T1-I-1: api/providers/claw_provider 单刀目录化 + 3 路抽出

## Status
- State: `done`

## Goal

T1-I 单刀完成。`api/providers/claw_provider.rs` (1224 LOC) → 4 文件目录：

1. `git mv claw_provider.rs → claw_provider/mod.rs`
2. tests block (~441 LOC) → `claw_provider/tests.rs`
3. `impl ClawApiClient` 内部 helpers + retry/backoff (~275 LOC) → `claw_provider/provider_impl.rs`
4. MessageStream + impl + expect_success + is_retryable_status + ApiErrorEnvelope/Body (~85 LOC) → `claw_provider/message_stream.rs`

mod.rs 留 AuthSource enum + impls (2 个) + OAuthTokenSet + ClawApiClient struct + build_http_client + 全部 pub fn (oauth_token_is_expired / resolve_saved_oauth_token / has_auth_from_env_or_saved / resolve_startup_auth_source / read_base_url / read_model_override / etc) + ClaudeCodeSettings helpers + impl Provider for ClawApiClient (~427 LOC, < 500 target ✓)。

## Files (scope)
- src-tauri/src/modules/api/providers/claw_provider.rs (deleted via mv)
- src-tauri/src/modules/api/providers/claw_provider/mod.rs
- src-tauri/src/modules/api/providers/claw_provider/provider_impl.rs
- src-tauri/src/modules/api/providers/claw_provider/message_stream.rs
- src-tauri/src/modules/api/providers/claw_provider/tests.rs

## Contract
- I1: pub struct ClawApiClient + pub fn (DEFAULT_BASE_URL etc) + pub MessageStream 全部留/re-export — 外部 importer 路径不变
- 8 个 const (ANTHROPIC_VERSION / REQUEST_ID_HEADER / DEFAULT_*) 升 pub(super)
- MessageStream 6 个字段升 pub(super) (provider_impl.rs 需直接构造 it)
- backoff_for_attempt 升 pub(super) (tests.rs 通过 super::* 访问)
- I3: HTTP request flow + error message strings + SSE parsing byte-identical
- I6: function bodies 一字不改

## Verify
- cargo build PASS clean
- cargo fmt clean
- cargo clippy --workspace --all-targets -D warnings GREEN
- cargo test --no-run PASS

## Done
- mod.rs 1224 → 427 LOC (-65%)
- 不需 SIZE_EXEMPT (under 500 target)
- T1-I arc 单刀完成
