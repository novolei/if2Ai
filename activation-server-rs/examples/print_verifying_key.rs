//! Print base64url(no-pad) Ed25519 **verifying** key from the server's
//! `SIGNING_KEY_B64` (same format as `activation-server-rs` `.env`).
//!
//! ```bash
//! export SIGNING_KEY_B64='…'   # from server .env (32-byte secret, base64url)
//! cargo run --example print_verifying_key
//! ```
//!
//! Paste the one-line output into if2Ai `keys/activation_license_ed25519_pub.b64`,
//! or set `IF2AI_LICENSE_ED25519_PUBKEY_B64` when building if2Ai (see that crate's
//! `build.rs`).

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::SigningKey;

fn main() {
    let b64 = std::env::var("SIGNING_KEY_B64").expect("SIGNING_KEY_B64 must be set");
    let secret = URL_SAFE_NO_PAD
        .decode(b64.trim().as_bytes())
        .expect("invalid base64url");
    let arr: [u8; 32] = secret
        .try_into()
        .expect("SIGNING_KEY_B64 must decode to exactly 32 bytes");
    let sk = SigningKey::from_bytes(&arr);
    println!("{}", URL_SAFE_NO_PAD.encode(sk.verifying_key().to_bytes()));
}
