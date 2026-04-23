activation_license_ed25519_pub.b64
====================================

One line: base64url (no padding), 32 raw bytes = Ed25519 verifying key
for the activation server's license JWS.

Default checked in: RFC 8032 test vector public key (matches unit tests only).

Production: on the host that has SIGNING_KEY_B64 in .env, run from
activation-server-rs:

  export SIGNING_KEY_B64='…'
  cargo run --example print_verifying_key

Replace this file with that output, OR set environment variable at
if2Ai build time:

  IF2AI_LICENSE_ED25519_PUBKEY_B64='<same line>' cargo build …

See src-tauri/build.rs and license_evaluator.rs.
