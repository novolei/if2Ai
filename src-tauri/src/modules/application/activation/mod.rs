//! Activation subsystem (Phase M2.6 — UClaw activation server port).
//!
//! Implements the **client side** of `iclaw-activation-server` v0.2
//! (axum + sqlite + ed25519-dalek).  Base URL：`IF2AI_ACTIVATION_BASE_URL`，
//! 未设置时使用 [`http_client::DEFAULT_ACTIVATION_BASE_URL`]。服务端源码见
//! 仓库内 `activation-server-rs/src/main.rs`。
//!
//! `license_jws` is verified with **Ed25519** against the verifying key
//! baked in at build time (`build.rs` + `keys/
//! activation_license_ed25519_pub.b64` or
//! `IF2AI_LICENSE_ED25519_PUBKEY_B64`).  Derive the pubkey line from the
//! server's `SIGNING_KEY_B64` via `activation-server-rs` example
//! `print_verifying_key`.  Forging only `~/.if2ai/activation/license.json`
//! without a matching server signature is rejected.
//!
//! Module map (god-file split, each file ≤ 500 LOC):
//! - [`models`]            — wire schema + local stored types + errors.
//! - [`installation_id`]   — stable per-device identifier + 8-char
//!                           human-friendly device indicator.
//! - [`license_store`]     — `LicenseStore` trait + atomic 0o600
//!                           file-backed implementation.
//! - [`license_evaluator`] — Ed25519 JWS verify + claim / clock binding.
//! - [`http_client`]       — `reqwest`-backed client with retry / 429 /
//!                           503 / `Retry-After` honoring + jitter.
//! - [`lifecycle_manager`] — boot-time + periodic refresh / revoke
//!                           coordinator (skeleton; not auto-started
//!                           in this Pack — `LicenseLifecycleService`
//!                           drives single-shot calls today).

pub mod http_client;
pub mod installation_id;
pub mod license_evaluator;
pub mod license_store;
pub mod lifecycle_manager;
pub mod models;
