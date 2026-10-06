#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs::{File, TryLockError};

use super::{lock_exclusive, write_secret_file};

#[cfg(unix)]
fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn writes_the_content_creating_parent_folders() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a/b/secret.json");

    write_secret_file(&path, b"{\"k\":1}").unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"k\":1}");
    // Written through a temporary sibling renamed over the target: none left behind.
    assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
}

#[test]
fn replaces_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secret.json");
    std::fs::write(&path, "old content that is longer").unwrap();

    write_secret_file(&path, b"new").unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}

#[cfg(unix)]
#[test]
fn new_file_is_readable_only_by_the_owner() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secret.json");

    write_secret_file(&path, b"x").unwrap();

    assert_eq!(mode(&path), 0o600);
}

#[cfg(unix)]
#[test]
fn existing_world_readable_file_ends_up_owner_only() {
    // Regression for #165: secrets were written with plain fs::write and kept
    // the umask's 0644, readable by every local user.
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secret.json");
    std::fs::write(&path, "x").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    // A leftover temporary file from an earlier crash keeps its old mode too.
    let tmp = dir.path().join("secret.json.tmp");
    std::fs::write(&tmp, "stale").unwrap();
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644)).unwrap();

    write_secret_file(&path, b"y").unwrap();

    assert_eq!(mode(&path), 0o600);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "y");
}

#[test]
fn lock_is_exclusive_until_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("users/alice/credentials.json");

    let guard = lock_exclusive(&path).unwrap();
    let lock_file = dir.path().join("users/alice/credentials.json.lock");
    let other = File::open(&lock_file).unwrap();
    assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));

    drop(guard);
    other.try_lock().unwrap();
}

#[test]
fn locks_on_different_files_are_independent() {
    let dir = tempfile::tempdir().unwrap();
    let _alice = lock_exclusive(&dir.path().join("users/alice/credentials.json")).unwrap();
    let _bob = lock_exclusive(&dir.path().join("users/bob/credentials.json")).unwrap();
}
