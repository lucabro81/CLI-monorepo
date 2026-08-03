#![allow(clippy::unwrap_used, clippy::expect_used)]

use tempfile::NamedTempFile;

use super::{resolve_body, resolve_optional_body};
use crate::error::CliError;

#[test]
fn resolve_body_accepts_body_only() {
    let resolved = resolve_body(Some("<p>hi</p>".to_string()), None).expect("should resolve");

    assert_eq!(resolved, "<p>hi</p>");
}

#[test]
fn resolve_body_accepts_body_file_only() {
    let file = NamedTempFile::new().expect("tempfile");
    std::fs::write(file.path(), "<p>from file</p>").expect("write");
    let path = file.path().to_str().expect("utf8 path").to_string();

    let resolved = resolve_body(None, Some(path)).expect("should resolve");

    assert_eq!(resolved, "<p>from file</p>");
}

#[test]
fn resolve_body_rejects_none_provided() {
    let result = resolve_body(None, None);

    assert!(matches!(result, Err(CliError::TemplateCreateMissingBodySource)));
}

#[test]
fn resolve_body_rejects_both_as_internal_error() {
    // Unreachable in practice — clap's conflicts_with rules this out at parse
    // time — but the function must still fail loudly rather than silently
    // picking one, in case that invariant is ever broken.
    let result = resolve_body(Some("<p>a</p>".to_string()), Some("./x.html".to_string()));

    assert!(matches!(result, Err(CliError::Internal(_))));
}

#[test]
fn resolve_body_file_not_found_is_io_error() {
    let result = resolve_body(None, Some("/does/not/exist.html".to_string()));

    assert!(matches!(result, Err(CliError::IoError { .. })));
}

#[test]
fn resolve_optional_body_returns_none_when_neither_given() {
    // Unlike resolve_body (template create, content mandatory), "neither
    // given" is a valid case here — it means "keep the current body".
    let resolved = resolve_optional_body(None, None).expect("should resolve");

    assert_eq!(resolved, None);
}

#[test]
fn resolve_optional_body_returns_some_with_body_only() {
    let resolved =
        resolve_optional_body(Some("<p>new</p>".to_string()), None).expect("should resolve");

    assert_eq!(resolved, Some("<p>new</p>".to_string()));
}

#[test]
fn resolve_optional_body_returns_some_with_body_file_only() {
    let file = NamedTempFile::new().expect("tempfile");
    std::fs::write(file.path(), "<p>from file</p>").expect("write");
    let path = file.path().to_str().expect("utf8 path").to_string();

    let resolved = resolve_optional_body(None, Some(path)).expect("should resolve");

    assert_eq!(resolved, Some("<p>from file</p>".to_string()));
}

#[test]
fn resolve_optional_body_rejects_both_as_internal_error() {
    let result = resolve_optional_body(Some("<p>a</p>".to_string()), Some("./x.html".to_string()));

    assert!(matches!(result, Err(CliError::Internal(_))));
}
