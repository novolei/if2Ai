//! License evaluator — local validity decision with **Ed25519 JWS
//! verification**.
//!
//! The activation server signs `license_jws` as
//! `base64url(header).base64url(payload).base64url(signature)` where
//! the signed bytes are **exactly** the UTF-8 string
//! `{header_b64}.{payload_b64}` (see `activation-server-rs`
//! `sign_license_jws`).
//!
//! The verifying key is baked in at build time (see crate `build.rs`):
//! `IF2AI_LICENSE_ED25519_PUBKEY_B64` or
//! `keys/activation_license_ed25519_pub.b64`.  Derive the line from
//! the server's `SIGNING_KEY_B64` via
//! `activation-server-rs` example `print_verifying_key`.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::DateTime;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use once_cell::sync::Lazy;
use serde::Deserialize;

use super::models::{LicenseClaims, LicenseValidity, StoredLicense};

/// Skew tolerance to absorb harmless local-clock drift.
const SKEW_TOLERANCE_SECS: i64 = 60;

/// Ed25519 verifying key for `license_jws` (32 bytes), materialized by `build.rs`.
static LICENSE_VERIFYING_KEY: Lazy<VerifyingKey> = Lazy::new(|| {
    let s = include_str!(concat!(env!("OUT_DIR"), "/activation_license_pub.b64")).trim();
    let bytes = URL_SAFE_NO_PAD
        .decode(s.as_bytes())
        .unwrap_or_else(|e| panic!("activation license pubkey (OUT_DIR): invalid base64url: {e}"));
    let arr: [u8; 32] = bytes.try_into().unwrap_or_else(|v: Vec<u8>| {
        panic!(
            "activation license pubkey must be 32 bytes, got {}",
            v.len()
        )
    });
    VerifyingKey::from_bytes(&arr).expect("built-in activation license pubkey is invalid Ed25519")
});

/// Minimal JWS header — we only insist on Ed25519 / EdDSA to avoid
/// cross-algorithm confusion if the server ever adds more key types.
#[derive(Debug, Deserialize)]
struct LicenseJwsHeader {
    alg: String,
}

/// Evaluate the local license against a current device + app id.
///
/// All comparisons happen against `now_trusted = max(local_now,
/// last_trusted_server_time) + SKEW_TOLERANCE_SECS` so a user who
/// rolls their system clock back into the validity window cannot
/// outlive the server-issued `exp`.
pub fn evaluate(
    stored: &StoredLicense,
    current_installation_id: &str,
    expected_app_id: &str,
) -> LicenseValidity {
    let claims = match verify_jws_and_decode_claims(&stored.license_jws) {
        Ok(c) => c,
        Err(VerifyJwsError::InvalidSignature) => return LicenseValidity::InvalidSignature,
        Err(VerifyJwsError::Malformed) => return LicenseValidity::MalformedJws,
    };

    if claims
        .installation_id
        .as_deref()
        .is_none_or(|c| c != current_installation_id || c != stored.installation_id)
    {
        return LicenseValidity::DeviceMismatch;
    }
    if claims.aud.as_deref() != Some(expected_app_id) {
        return LicenseValidity::AudienceMismatch;
    }

    let exp = match claims.exp {
        Some(v) => v,
        None => return LicenseValidity::MalformedJws,
    };
    let grace = claims.offline_grace_exp.unwrap_or(exp);
    let now_trusted = compute_now_trusted(&stored.last_trusted_server_time) + SKEW_TOLERANCE_SECS;

    if now_trusted <= exp {
        LicenseValidity::Valid
    } else if now_trusted <= grace {
        LicenseValidity::OfflineGrace
    } else {
        LicenseValidity::Expired
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyJwsError {
    Malformed,
    InvalidSignature,
}

/// Verify Ed25519 signature on `license_jws` and decode claims from the payload.
fn verify_jws_and_decode_claims(jws: &str) -> Result<LicenseClaims, VerifyJwsError> {
    let mut parts = jws.split('.');
    let header_b64 = parts.next().ok_or(VerifyJwsError::Malformed)?;
    let payload_b64 = parts.next().ok_or(VerifyJwsError::Malformed)?;
    let sig_b64 = parts.next().ok_or(VerifyJwsError::Malformed)?;
    if parts.next().is_some() {
        return Err(VerifyJwsError::Malformed);
    }

    let header_bytes = URL_SAFE_NO_PAD
        .decode(header_b64.as_bytes())
        .map_err(|_| VerifyJwsError::Malformed)?;
    let header: LicenseJwsHeader =
        serde_json::from_slice(&header_bytes).map_err(|_| VerifyJwsError::Malformed)?;
    if header.alg != "EdDSA" {
        return Err(VerifyJwsError::Malformed);
    }

    let signing_input = format!("{header_b64}.{payload_b64}");
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64.as_bytes())
        .map_err(|_| VerifyJwsError::Malformed)?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| VerifyJwsError::Malformed)?;
    let signature = Signature::from_slice(&sig_arr).map_err(|_| VerifyJwsError::Malformed)?;

    LICENSE_VERIFYING_KEY
        .verify(signing_input.as_bytes(), &signature)
        .map_err(|_| VerifyJwsError::InvalidSignature)?;

    let payload = URL_SAFE_NO_PAD
        .decode(payload_b64.as_bytes())
        .map_err(|_| VerifyJwsError::Malformed)?;
    serde_json::from_slice::<LicenseClaims>(&payload).map_err(|_| VerifyJwsError::Malformed)
}

/// Split a `header.payload.signature` JWS and base64url-decode the
/// payload **without** verifying the signature.
///
/// Only safe after [`evaluate`] has accepted the license, or for
/// display-only summaries of an already-validated document.
pub fn decode_jws_claims(jws: &str) -> Option<LicenseClaims> {
    let mut parts = jws.split('.');
    let _header = parts.next()?;
    let payload_b64 = parts.next()?;
    let _sig = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD.decode(payload_b64.as_bytes()).ok()?;
    serde_json::from_slice::<LicenseClaims>(&bytes).ok()
}

fn compute_now_trusted(last_trusted_iso: &str) -> i64 {
    let local_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let trusted = DateTime::parse_from_rfc3339(last_trusted_iso)
        .map(|dt| dt.timestamp())
        .unwrap_or(0);
    local_now.max(trusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;
    use ed25519_dalek::SigningKey;
    use serde_json::json;

    /// RFC 8032 / Wycheproof-style Ed25519 secret (32 bytes).  Its
    /// public key only matches `keys/activation_license_ed25519_pub.b64`
    /// when that file is the placeholder used for offline tests; in
    /// production the file holds the verifying key for the activation
    /// server's `SIGNING_KEY_B64`, so we guard signature-dependent
    /// tests with [`have_matching_pubkey`] below.
    fn rfc8032_signing_key() -> SigningKey {
        let secret: [u8; 32] = [
            0x9d, 0x61, 0xb1, 0x9d, 0x03, 0x5f, 0x4a, 0x82, 0xd5, 0xf3, 0xc9, 0xce, 0xdf, 0x0f,
            0x65, 0x47, 0x3f, 0x9f, 0xce, 0x8e, 0xcc, 0x17, 0x73, 0xd7, 0xbf, 0x87, 0x9b, 0x2c,
            0xf9, 0x71, 0x56, 0xad,
        ];
        SigningKey::from_bytes(&secret)
    }

    /// Skip helper: when the embedded license pubkey does not match the
    /// RFC 8032 test private key (i.e. we're building with the real
    /// production key), tests that need to sign a JWS locally are
    /// skipped rather than failing. The signature-independent tests
    /// (`malformed_jws_returns_malformed`) still run unconditionally.
    fn have_matching_pubkey() -> bool {
        rfc8032_signing_key().verifying_key().to_bytes() == LICENSE_VERIFYING_KEY.to_bytes()
    }

    fn sign_license_jws_like_server(sk: &SigningKey, claims: serde_json::Value) -> String {
        let header = json!({
            "alg": "EdDSA",
            "typ": "JWT",
            "kid": "k1"
        });
        let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
        let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        let signing_input = format!("{header_b64}.{payload_b64}");
        let sig = sk.sign(signing_input.as_bytes());
        let sig_b64 = URL_SAFE_NO_PAD.encode(sig.to_bytes());
        format!("{signing_input}.{sig_b64}")
    }

    fn stored(jws: &str, installation: &str) -> StoredLicense {
        StoredLicense {
            license_id: "lic_t".into(),
            refresh_token: "rt_t".into(),
            license_jws: jws.into(),
            installation_id: installation.into(),
            last_trusted_server_time: "2020-01-01T00:00:00+00:00".into(),
        }
    }

    #[test]
    fn valid_when_now_before_exp() {
        if !have_matching_pubkey() {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let sk = rfc8032_signing_key();
        let jws = sign_license_jws_like_server(
            &sk,
            json!({
                "aud": "ai.if2.if2Ai",
                "installation_id": "sha256_a",
                "exp": now + 3600,
                "offline_grace_exp": now + 7200,
            }),
        );
        assert_eq!(
            evaluate(&stored(&jws, "sha256_a"), "sha256_a", "ai.if2.if2Ai"),
            LicenseValidity::Valid
        );
    }

    #[test]
    fn offline_grace_when_past_exp_but_before_grace() {
        if !have_matching_pubkey() {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let sk = rfc8032_signing_key();
        let jws = sign_license_jws_like_server(
            &sk,
            json!({
                "aud": "ai.if2.if2Ai",
                "installation_id": "sha256_a",
                "exp": now - 100,
                "offline_grace_exp": now + 3600,
            }),
        );
        assert_eq!(
            evaluate(&stored(&jws, "sha256_a"), "sha256_a", "ai.if2.if2Ai"),
            LicenseValidity::OfflineGrace
        );
    }

    #[test]
    fn expired_when_past_grace() {
        if !have_matching_pubkey() {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let sk = rfc8032_signing_key();
        let jws = sign_license_jws_like_server(
            &sk,
            json!({
                "aud": "ai.if2.if2Ai",
                "installation_id": "sha256_a",
                "exp": now - 7200,
                "offline_grace_exp": now - 3600,
            }),
        );
        assert_eq!(
            evaluate(&stored(&jws, "sha256_a"), "sha256_a", "ai.if2.if2Ai"),
            LicenseValidity::Expired
        );
    }

    #[test]
    fn device_mismatch_when_installation_differs() {
        if !have_matching_pubkey() {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let sk = rfc8032_signing_key();
        let jws = sign_license_jws_like_server(
            &sk,
            json!({
                "aud": "ai.if2.if2Ai",
                "installation_id": "sha256_a",
                "exp": now + 3600,
                "offline_grace_exp": now + 7200,
            }),
        );
        assert_eq!(
            evaluate(&stored(&jws, "sha256_a"), "sha256_OTHER", "ai.if2.if2Ai"),
            LicenseValidity::DeviceMismatch
        );
    }

    #[test]
    fn audience_mismatch_when_aud_differs() {
        if !have_matching_pubkey() {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let sk = rfc8032_signing_key();
        let jws = sign_license_jws_like_server(
            &sk,
            json!({
                "aud": "com.wt.iClaw",
                "installation_id": "sha256_a",
                "exp": now + 3600,
                "offline_grace_exp": now + 7200,
            }),
        );
        assert_eq!(
            evaluate(&stored(&jws, "sha256_a"), "sha256_a", "ai.if2.if2Ai"),
            LicenseValidity::AudienceMismatch
        );
    }

    #[test]
    fn malformed_jws_returns_malformed() {
        assert_eq!(
            evaluate(&stored("not-a-jws", "sha256_a"), "sha256_a", "ai.if2.if2Ai"),
            LicenseValidity::MalformedJws
        );
    }

    #[test]
    fn bad_signature_returns_invalid_signature() {
        if !have_matching_pubkey() {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let sk = rfc8032_signing_key();
        let jws = sign_license_jws_like_server(
            &sk,
            json!({
                "aud": "ai.if2.if2Ai",
                "installation_id": "sha256_a",
                "exp": now + 3600,
                "offline_grace_exp": now + 7200,
            }),
        );
        let parts: Vec<&str> = jws.split('.').collect();
        assert_eq!(parts.len(), 3);
        let mut sig = URL_SAFE_NO_PAD.decode(parts[2].as_bytes()).unwrap();
        sig[7] ^= 0xFF;
        let tampered = format!("{}.{}.{}", parts[0], parts[1], URL_SAFE_NO_PAD.encode(&sig));

        assert_eq!(
            evaluate(&stored(&tampered, "sha256_a"), "sha256_a", "ai.if2.if2Ai"),
            LicenseValidity::InvalidSignature
        );
    }

    #[test]
    fn rolled_back_clock_does_not_revive_expired_license() {
        if !have_matching_pubkey() {
            return;
        }
        let sk = rfc8032_signing_key();
        let jws = sign_license_jws_like_server(
            &sk,
            json!({
                "aud": "ai.if2.if2Ai",
                "installation_id": "sha256_a",
                "exp": 4_070_908_800_i64,
                "offline_grace_exp": 4_070_995_200_i64,
            }),
        );
        let mut s = stored(&jws, "sha256_a");
        s.last_trusted_server_time = "2099-06-01T00:00:00+00:00".into();
        assert_eq!(
            evaluate(&s, "sha256_a", "ai.if2.if2Ai"),
            LicenseValidity::Expired
        );
    }
}
