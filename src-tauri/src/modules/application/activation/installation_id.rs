//! Stable per-device installation identifier + 8-char human indicator.
//!
//! The installation id is the **only** value the activation server
//! uses to bind a license to a device.  Stability requirements:
//!
//! - Same value across app restarts.
//! - Same value across upgrades (within the `~/.if2ai/` data root).
//! - Different value on a fresh install / wiped data root (this is
//!   acceptable — the user re-redeems an invite code).
//!
//! Implementation: SHA-256 of `app_id + "::" + sys-locale-machine-id`,
//! stored once in `~/.if2ai/activation/installation_id` so subsequent
//! runs return the cached value byte-for-byte.  When the cache is
//! missing we derive from `whoami` + `hostname` + `OS` so two different
//! macOS users on the same machine get different ids.
//!
//! The 8-char `device_indicator` is a short, human-friendly slice used
//! by the activation modal's bottom-right hint (and by the admin UI to
//! match a user's screenshot to a request).

use sha2::{Digest, Sha256};
use std::path::PathBuf;

use crate::modules::config::store::if2ai_data_root;

/// Folder that holds activation-related state.
pub fn activation_dir() -> PathBuf {
    if2ai_data_root().join("activation")
}

/// File backing the cached installation id.
pub fn installation_id_path() -> PathBuf {
    activation_dir().join("installation_id")
}

/// File backing the on-disk license cache.
pub fn license_path() -> PathBuf {
    activation_dir().join("license.json")
}

/// Return the stable installation id for this device.
///
/// Caches the first computation under [`installation_id_path`].
pub fn installation_id() -> String {
    if let Ok(cached) = std::fs::read_to_string(installation_id_path()) {
        let trimmed = cached.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let derived = derive_installation_id();
    let _ = persist_installation_id(&derived);
    derived
}

/// 8-char uppercase indicator derived from the installation id.
///
/// **Strictly mirrors UClaw's `ActivationDeviceIndicator.display(from:)`**
/// (`UClawApp/.../ActivationDeviceIndicator.swift`):
/// 1. `SHA-256(installation_id_utf8_bytes)`
/// 2. Stream the digest as big-endian bits and pull out the first
///    eight 5-bit groups.
/// 3. Map each group through the Crockford-style alphabet
///    `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (no I, L, O, 0, 1, S vs 5
///    confusion is reduced by dropping I/O/0/1).
/// 4. Pad with `X` if we ever run out of bits (digest is 32 bytes,
///    so this never triggers in practice).
/// 5. Format `XXXX-XXXX`.
pub fn device_indicator(installation_id: &str) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

    let digest = Sha256::digest(installation_id.as_bytes());

    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;
    let mut out: Vec<u8> = Vec::with_capacity(8);

    for byte in digest.iter() {
        buffer = (buffer << 8) | u64::from(*byte);
        bits += 8;
        while bits >= 5 && out.len() < 8 {
            let index = ((buffer >> (bits - 5)) & 0x1F) as usize;
            out.push(ALPHABET[index]);
            bits -= 5;
        }
        if out.len() == 8 {
            break;
        }
    }

    while out.len() < 8 {
        out.push(b'X');
    }

    let s = std::str::from_utf8(&out).expect("alphabet is ASCII");
    format!("{}-{}", &s[..4], &s[4..])
}

fn derive_installation_id() -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"if2ai::activation::v1::");

    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".into());
    hasher.update(username.as_bytes());
    hasher.update(b"::");

    let hostname = hostname_lossy();
    hasher.update(hostname.as_bytes());
    hasher.update(b"::");

    hasher.update(std::env::consts::OS.as_bytes());
    hasher.update(b"::");
    hasher.update(std::env::consts::ARCH.as_bytes());

    let digest = hasher.finalize();
    format!("sha256_{}", hex::encode(digest))
}

fn hostname_lossy() -> String {
    // Cross-platform best-effort. Falls back to env vars if `gethostname`-style
    // syscall isn't available; we don't take an extra dependency just for this.
    if let Ok(h) = std::env::var("HOSTNAME") {
        if !h.is_empty() {
            return h;
        }
    }
    if let Ok(h) = std::env::var("COMPUTERNAME") {
        if !h.is_empty() {
            return h;
        }
    }
    // POSIX uname via libc is heavy; we accept "host-unknown" rather than
    // pulling in another crate. Different users on the same host still get
    // distinct ids via $USER above.
    "host-unknown".to_string()
}

fn persist_installation_id(value: &str) -> std::io::Result<()> {
    let dir = activation_dir();
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dir)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&dir, perms)?;
    }
    let path = installation_id_path();
    std::fs::write(&path, value)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_indicator_is_stable_and_well_formed() {
        let id = "sha256_deadbeefcafef00d1234567890abcdefdeadbeefcafef00d1234567890abcd";
        let indicator = device_indicator(id);
        assert_eq!(indicator.len(), 9); // 4 + '-' + 4
        assert_eq!(&indicator[4..5], "-");
        const ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        assert!(indicator.chars().all(|c| c == '-' || ALPHABET.contains(c)));
        assert_eq!(device_indicator(id), indicator); // deterministic
    }

    /// Pinned parity with UClaw's `ActivationDeviceIndicator.display`.
    /// Reference values cross-checked with a Python implementation of
    /// the exact same algorithm (`sha256(input).digest()` + Crockford
    /// alphabet `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`).
    #[test]
    fn device_indicator_matches_uclaw_algorithm_for_known_inputs() {
        assert_eq!(device_indicator("uclaw-test"), "E7SW-6465");
        assert_eq!(device_indicator("sha256_abcdef"), "BCLL-FFJS");
    }

    #[test]
    fn derived_id_is_prefixed_and_hex() {
        let id = derive_installation_id();
        assert!(id.starts_with("sha256_"));
        assert_eq!(id.len(), 7 + 64);
    }
}
