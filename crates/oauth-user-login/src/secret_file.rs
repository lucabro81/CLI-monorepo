//! Files holding secrets (credentials, `app.json`, the pending login): written
//! atomically with owner-only permissions, and an exclusive lock so two
//! processes renewing the same credentials don't race (issue #175).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Writes `bytes` to `path` with mode 0600 (on Unix), creating parent folders.
/// The content goes to a `<file>.tmp` sibling renamed over the target, so a
/// concurrent reader never sees a half-written file; an existing file or
/// leftover temporary file with looser permissions ends up 0600 too.
pub fn write_secret_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = sibling(path, "tmp");
    owner_only_options().truncate(true).open(&tmp)?.write_all(bytes)?;
    #[cfg(unix)]
    {
        // `mode` only applies when the file is created: a leftover keeps its old one.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)
}

/// An exclusive lock on `<path>.lock`, released when dropped.
#[derive(Debug)]
pub struct FileLock {
    _file: File,
}

/// Blocks until this process holds the exclusive lock guarding `path`
/// (`<path>.lock`, created with its parent folders if missing). Locks on
/// different paths are independent.
pub fn lock_exclusive(path: &Path) -> std::io::Result<FileLock> {
    let lock_path = sibling(path, "lock");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = owner_only_options().open(&lock_path)?;
    file.lock()?;
    Ok(FileLock { _file: file })
}

fn owner_only_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

/// `<path>.<extension>`, keeping the original extension (`a.json` → `a.json.lock`).
fn sibling(path: &Path, extension: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".");
    name.push(extension);
    PathBuf::from(name)
}

#[cfg(test)]
#[path = "tests/secret_file_tests.rs"]
mod tests;
