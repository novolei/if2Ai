//! Copies the activation-license Ed25519 **public** key (base64url, no pad,
//! 32 raw bytes) into `OUT_DIR` so `license_evaluator` can `include_str!` it.
//!
//! Resolution order:
//! 1. `IF2AI_LICENSE_ED25519_PUBKEY_B64` — used by release CI without
//!    committing production keys to git.
//! 2. Otherwise `keys/activation_license_ed25519_pub.b64` next to this
//!    manifest (default: RFC 8032 test vector public key for unit tests).

use std::env;
use std::path::PathBuf;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

fn main() {
    materialize_activation_license_pubkey();
    tauri_build::build();
}

fn materialize_activation_license_pubkey() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let dest = out_dir.join("activation_license_pub.b64");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));

    let raw = if let Ok(v) = env::var("IF2AI_LICENSE_ED25519_PUBKEY_B64") {
        v
    } else {
        let path = manifest_dir.join("keys/activation_license_ed25519_pub.b64");
        std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "missing keys/activation_license_ed25519_pub.b64 under {}; \
                 set IF2AI_LICENSE_ED25519_PUBKEY_B64 or add the file",
                manifest_dir.display()
            )
        })
    };

    let trimmed = raw.trim();
    let decoded = URL_SAFE_NO_PAD
        .decode(trimmed.as_bytes())
        .unwrap_or_else(|e| panic!("activation license pubkey: invalid base64url: {e}"));
    assert_eq!(
        decoded.len(),
        32,
        "activation license pubkey must decode to 32 bytes (Ed25519), got {} bytes",
        decoded.len()
    );

    std::fs::write(&dest, trimmed).expect("write OUT_DIR/activation_license_pub.b64");
    println!("cargo:rerun-if-changed=keys/activation_license_ed25519_pub.b64");
    println!("cargo:rerun-if-env-changed=IF2AI_LICENSE_ED25519_PUBKEY_B64");
}
