#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{apply_stale_filter, parse_body_segments, validate_assign_target, BodySegment};

#[test]
fn apply_stale_filter_returns_jql_unchanged_when_stale_days_is_none() {
    assert_eq!(apply_stale_filter("project=KAN", None), "project=KAN");
}

#[test]
fn apply_stale_filter_appends_clause_when_no_order_by() {
    assert_eq!(
        apply_stale_filter("project=KAN AND status!=Done", Some(14)),
        "project=KAN AND status!=Done AND updated <= -14d"
    );
}

#[test]
fn apply_stale_filter_inserts_clause_before_order_by() {
    assert_eq!(
        apply_stale_filter("project=KAN ORDER BY created DESC", Some(7)),
        "project=KAN AND updated <= -7d ORDER BY created DESC"
    );
}

#[test]
fn apply_stale_filter_matches_order_by_case_insensitively() {
    assert_eq!(
        apply_stale_filter("project=KAN order by created desc", Some(7)),
        "project=KAN AND updated <= -7d order by created desc"
    );
}

#[test]
fn apply_stale_filter_zero_days_is_still_valid_jql() {
    assert_eq!(
        apply_stale_filter("project=KAN", Some(0)),
        "project=KAN AND updated <= -0d"
    );
}

#[test]
fn parse_body_segments_returns_empty_vec_for_empty_body() {
    assert_eq!(parse_body_segments(""), vec![]);
}

#[test]
fn parse_body_segments_returns_single_text_segment_when_no_placeholder() {
    assert_eq!(
        parse_body_segments("just plain text"),
        vec![BodySegment::Text("just plain text".to_string())]
    );
}

#[test]
fn parse_body_segments_returns_single_mention_when_body_is_only_a_placeholder() {
    assert_eq!(
        parse_body_segments("{{mention:5b10ac8d}}"),
        vec![BodySegment::Mention("5b10ac8d".to_string())]
    );
}

#[test]
fn parse_body_segments_splits_text_around_a_mid_sentence_placeholder() {
    assert_eq!(
        parse_body_segments("Thanks {{mention:5b10ac8d}} for the fix"),
        vec![
            BodySegment::Text("Thanks ".to_string()),
            BodySegment::Mention("5b10ac8d".to_string()),
            BodySegment::Text(" for the fix".to_string()),
        ]
    );
}

#[test]
fn parse_body_segments_handles_placeholder_at_start() {
    assert_eq!(
        parse_body_segments("{{mention:5b10ac8d}} thanks"),
        vec![
            BodySegment::Mention("5b10ac8d".to_string()),
            BodySegment::Text(" thanks".to_string()),
        ]
    );
}

#[test]
fn parse_body_segments_handles_placeholder_at_end() {
    assert_eq!(
        parse_body_segments("thanks {{mention:5b10ac8d}}"),
        vec![
            BodySegment::Text("thanks ".to_string()),
            BodySegment::Mention("5b10ac8d".to_string()),
        ]
    );
}

#[test]
fn parse_body_segments_handles_back_to_back_placeholders_with_no_text_between() {
    assert_eq!(
        parse_body_segments("{{mention:aaa}}{{mention:bbb}}"),
        vec![
            BodySegment::Mention("aaa".to_string()),
            BodySegment::Mention("bbb".to_string()),
        ]
    );
}

#[test]
fn parse_body_segments_handles_multiple_placeholders_with_text_between() {
    assert_eq!(
        parse_body_segments("hi {{mention:aaa}} and {{mention:bbb}} bye"),
        vec![
            BodySegment::Text("hi ".to_string()),
            BodySegment::Mention("aaa".to_string()),
            BodySegment::Text(" and ".to_string()),
            BodySegment::Mention("bbb".to_string()),
            BodySegment::Text(" bye".to_string()),
        ]
    );
}

#[test]
fn parse_body_segments_preserves_colons_inside_account_id() {
    // Some Jira account IDs (Connect app users) contain colons, e.g. "qm:1234:abcd".
    // The parser must not treat the colon as a delimiter.
    assert_eq!(
        parse_body_segments("{{mention:qm:1234:abcd}}"),
        vec![BodySegment::Mention("qm:1234:abcd".to_string())]
    );
}

#[test]
fn parse_body_segments_treats_unterminated_placeholder_as_literal_text() {
    // Missing closing "}}" — an LLM-generated malformed placeholder should not
    // silently swallow the rest of the comment; it is kept as plain text instead.
    assert_eq!(
        parse_body_segments("hello {{mention:no-closing-brace"),
        vec![BodySegment::Text("hello {{mention:no-closing-brace".to_string())]
    );
}

#[test]
fn validate_assign_target_ok_with_assignee_only() {
    assert!(validate_assign_target("KAN-5", Some("account-id-123"), false).is_ok());
}

#[test]
fn validate_assign_target_ok_with_unassign_only() {
    assert!(validate_assign_target("KAN-5", None, true).is_ok());
}

#[test]
fn validate_assign_target_err_with_neither_assignee_nor_unassign() {
    // clap's `conflicts_with` rules out passing both --assignee and --unassign
    // together, but does not require at least one of them — this runtime check
    // covers the "neither" case, e.g. `jira issue assign KAN-5` alone.
    let result = validate_assign_target("KAN-5", None, false);
    assert!(result.is_err());
}
