//! PKCE (RFC 7636, S256) and the CSRF `state` value.

/// PKCE code verifier: 64 random bytes, base64url without padding (86 chars,
/// within RFC 7636's 43-128).
pub fn generate_code_verifier() -> String {
    random_url_safe(64)
}

/// PKCE S256 challenge: base64url(sha256(verifier)), no padding.
pub fn code_challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Opaque CSRF `state` for the authorize request: 32 random bytes, base64url.
pub fn generate_state() -> String {
    random_url_safe(32)
}

fn random_url_safe(byte_len: usize) -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut bytes = vec![0u8; byte_len];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
#[path = "tests/pkce_tests.rs"]
mod tests;
