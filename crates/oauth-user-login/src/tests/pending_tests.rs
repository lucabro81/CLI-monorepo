#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use super::{
    PENDING_LOGIN_TTL_SECS, PendingLogin, PendingLoginError, PendingLoginStatus, pending_login_status,
    rfc3339_utc, take_pending_login,
};
use crate::code_challenge;

const NOW: u64 = 1_800_000_000;
const URI: &str = "https://mercury.example.com/oauth/callback";

fn file(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("cli-dir").join("pending-login.json")
}

fn saved(path: &Path, with_pkce: bool) -> PendingLogin {
    let pending = PendingLogin::new(Some(URI), with_pkce, NOW);
    pending.save(path).unwrap();
    pending
}

// ── new ───────────────────────────────────────────────────────────────────

#[test]
fn new_with_pkce_has_verifier_redirect_and_ten_minute_expiry() {
    let pending = PendingLogin::new(Some(URI), true, NOW);

    assert_eq!(pending.redirect_uri.as_deref(), Some(URI));
    assert_eq!(pending.expires_at, NOW + 600);
    assert_eq!(PENDING_LOGIN_TTL_SECS, 600);
    assert_eq!(pending.state.len(), 43);
    let verifier = pending.code_verifier.clone().unwrap();
    assert_eq!(verifier.len(), 86);
    assert_eq!(pending.code_challenge(), Some(code_challenge(&verifier)));
}

#[test]
fn new_without_pkce_has_no_verifier_and_no_challenge() {
    let pending = PendingLogin::new(Some(URI), false, NOW);

    assert_eq!(pending.code_verifier, None);
    assert_eq!(pending.code_challenge(), None);
}

// Bitbucket has no redirect_uri parameter: the consumer's callback URL decides.
#[test]
fn new_without_redirect_uri_or_pkce_keeps_only_state_and_expiry() {
    let pending = PendingLogin::new(None, false, NOW);

    assert_eq!(pending.redirect_uri, None);
    assert_eq!(pending.code_verifier, None);
    assert_eq!(pending.state.len(), 43);
    assert_eq!(pending.expires_at, NOW + 600);
}

#[test]
fn every_pending_login_gets_fresh_secrets() {
    let a = PendingLogin::new(Some(URI), true, NOW);
    let b = PendingLogin::new(Some(URI), true, NOW);

    assert_ne!(a.state, b.state);
    assert_ne!(a.code_verifier, b.code_verifier);
}

// ── save ──────────────────────────────────────────────────────────────────

#[test]
fn save_creates_parent_dirs_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    let pending = saved(&path, true);

    let on_disk: PendingLogin = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk, pending);
}

#[cfg(unix)]
#[test]
fn save_writes_owner_only_permissions_even_over_a_looser_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    saved(&path, true);

    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn a_new_pending_login_replaces_the_previous_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    let first = saved(&path, true);
    let second = saved(&path, true);

    assert_eq!(take_pending_login(&path, &first.state, NOW), Err(PendingLoginError::StateMismatch));
    assert_eq!(take_pending_login(&path, &second.state, NOW), Ok(second));
}

// ── take ──────────────────────────────────────────────────────────────────

#[test]
fn take_returns_the_login_and_removes_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    let pending = saved(&path, true);

    assert_eq!(take_pending_login(&path, &pending.state, NOW + 599), Ok(pending));
    assert!(!path.exists());
}

#[test]
fn state_is_single_use() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    let pending = saved(&path, true);
    take_pending_login(&path, &pending.state, NOW).unwrap();

    assert_eq!(take_pending_login(&path, &pending.state, NOW), Err(PendingLoginError::NotFound));
}

#[test]
fn take_without_a_pending_login_is_not_found() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(take_pending_login(&file(&dir), "any", NOW), Err(PendingLoginError::NotFound));
}

#[test]
fn a_mismatched_state_keeps_the_pending_login() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    let pending = saved(&path, true);

    assert_eq!(take_pending_login(&path, "stale", NOW), Err(PendingLoginError::StateMismatch));
    assert_eq!(take_pending_login(&path, &pending.state, NOW), Ok(pending));
}

#[test]
fn an_expired_pending_login_is_rejected_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    let pending = saved(&path, true);

    // Valid up to, not including, expires_at.
    assert_eq!(take_pending_login(&path, &pending.state, NOW + 600), Err(PendingLoginError::Expired));
    assert!(!path.exists());
}

#[test]
fn an_expired_pending_login_with_the_wrong_state_reports_the_mismatch_first() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    saved(&path, true);

    assert_eq!(take_pending_login(&path, "stale", NOW + 601), Err(PendingLoginError::StateMismatch));
    assert!(path.exists());
}

#[test]
fn a_corrupt_file_is_reported_and_left_for_step_one_to_replace() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not json").unwrap();

    assert!(matches!(take_pending_login(&path, "s", NOW), Err(PendingLoginError::Corrupt(_))));
    assert!(matches!(pending_login_status(&path, NOW), Err(PendingLoginError::Corrupt(_))));
    assert!(path.exists());
}

// Regression: two step-2 runs racing on the same pending login both matched
// the state; the loser failed to remove the file and reported an I/O error
// instead of "no pending login".
#[test]
fn losing_a_race_to_the_same_pending_login_reports_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    let pending = saved(&path, true);
    let winner = take_pending_login(&path, &pending.state, NOW);
    assert!(winner.is_ok());

    // The loser read the file before the winner removed it.
    assert_eq!(super::consume(&path), Err(PendingLoginError::NotFound));
}

#[test]
fn save_leaves_no_temporary_file_behind() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    saved(&path, true);

    let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["pending-login.json".to_string()]);
}

// ── status ────────────────────────────────────────────────────────────────

#[test]
fn status_reports_none_live_and_expired() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(&dir);
    assert_eq!(pending_login_status(&path, NOW), Ok(None));

    saved(&path, false);

    assert_eq!(
        pending_login_status(&path, NOW + 1),
        Ok(Some(PendingLoginStatus { expires_at: NOW + 600, expired: false }))
    );
    assert_eq!(
        pending_login_status(&path, NOW + 600),
        Ok(Some(PendingLoginStatus { expires_at: NOW + 600, expired: true }))
    );
    // Reading the status never consumes the pending login.
    assert!(path.exists());
}

// ── rfc3339_utc ───────────────────────────────────────────────────────────

#[test]
fn rfc3339_formats_known_instants() {
    assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(rfc3339_utc(1_800_000_000), "2027-01-15T08:00:00Z");
    assert_eq!(rfc3339_utc(4_102_444_799), "2099-12-31T23:59:59Z");
}

#[test]
fn error_messages_are_plain_and_specific() {
    assert_eq!(
        PendingLoginError::NotFound.to_string(),
        "there is no pending remote login in this config folder (none was started, or it was already completed)"
    );
    assert_eq!(
        PendingLoginError::Expired.to_string(),
        "the pending remote login expired (it is valid for 10 minutes after it was started)"
    );
    assert_eq!(
        PendingLoginError::StateMismatch.to_string(),
        "the state does not match the pending remote login (the link is from an older attempt, or \
        the state was mistyped). Use the code and state of the latest link"
    );
    assert_eq!(
        PendingLoginError::Corrupt("eof".to_string()).to_string(),
        "the pending remote login file is unreadable (eof)"
    );
    assert_eq!(
        PendingLoginError::Io("denied".to_string()).to_string(),
        "could not write or remove the pending remote login file (denied)"
    );
}
