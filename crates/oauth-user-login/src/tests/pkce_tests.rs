#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{code_challenge, generate_code_verifier, generate_state};

fn is_url_safe(value: &str) -> bool {
    value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[test]
fn code_challenge_matches_rfc7636_appendix_b_example() {
    assert_eq!(
        code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn code_verifier_is_86_url_safe_chars_within_rfc_bounds() {
    let verifier = generate_code_verifier();

    // 64 random bytes, base64url without padding.
    assert_eq!(verifier.len(), 86);
    assert!(is_url_safe(&verifier), "got {verifier}");
}

#[test]
fn code_verifiers_are_random() {
    assert_ne!(generate_code_verifier(), generate_code_verifier());
}

#[test]
fn code_challenge_is_url_safe_and_unpadded() {
    let challenge = code_challenge(&generate_code_verifier());

    // sha256 = 32 bytes -> 43 base64url chars without padding.
    assert_eq!(challenge.len(), 43);
    assert!(is_url_safe(&challenge), "got {challenge}");
}

#[test]
fn state_is_43_url_safe_chars_and_random() {
    let state = generate_state();

    // 32 random bytes, base64url without padding.
    assert_eq!(state.len(), 43);
    assert!(is_url_safe(&state), "got {state}");
    assert_ne!(state, generate_state());
}
