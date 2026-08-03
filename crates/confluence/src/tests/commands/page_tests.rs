#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{parse_body_source, validate_update_target, BodySource};
use crate::error::CliError;

#[test]
fn parse_body_source_accepts_body_only() {
    let source = parse_body_source(Some("<p>hi</p>".to_string()), None, None).expect("should parse");

    assert_eq!(source, BodySource::Body("<p>hi</p>".to_string()));
}

#[test]
fn parse_body_source_accepts_body_file_only() {
    let source =
        parse_body_source(None, Some("./content.html".to_string()), None).expect("should parse");

    assert_eq!(source, BodySource::BodyFile("./content.html".to_string()));
}

#[test]
fn parse_body_source_accepts_template_id_only() {
    let source = parse_body_source(None, None, Some("4321".to_string())).expect("should parse");

    assert_eq!(source, BodySource::TemplateId("4321".to_string()));
}

#[test]
fn parse_body_source_rejects_none_provided() {
    let result = parse_body_source(None, None, None);

    assert!(matches!(result, Err(CliError::PageCreateMissingBodySource)));
}

#[test]
fn parse_body_source_rejects_more_than_one_as_internal_error() {
    // Unreachable in practice — clap's conflicts_with_all rules this out at
    // parse time — but the function must still fail loudly rather than
    // silently picking one, in case that invariant is ever broken.
    let result = parse_body_source(
        Some("<p>hi</p>".to_string()),
        Some("./content.html".to_string()),
        None,
    );

    assert!(matches!(result, Err(CliError::Internal(_))));
}

#[test]
fn validate_update_target_ok_with_title_only() {
    assert!(validate_update_target(Some("New title"), None).is_ok());
}

#[test]
fn validate_update_target_ok_with_body_only() {
    assert!(validate_update_target(None, Some("<p>new</p>")).is_ok());
}

#[test]
fn validate_update_target_ok_with_both() {
    assert!(validate_update_target(Some("New title"), Some("<p>new</p>")).is_ok());
}

#[test]
fn validate_update_target_err_with_neither() {
    let result = validate_update_target(None, None);

    assert!(matches!(result, Err(CliError::PageUpdateMissingTarget)));
}
