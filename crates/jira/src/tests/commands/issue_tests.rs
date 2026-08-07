#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{
    apply_stale_filter, build_browse_url, expand_mentions_in_content, parse_body_segments,
    validate_assign_target, BodySegment,
};
use serde_json::json;

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
fn build_browse_url_joins_site_url_and_key() {
    assert_eq!(
        build_browse_url(Some("https://mysite.atlassian.net"), "KAN-1"),
        Some("https://mysite.atlassian.net/browse/KAN-1".to_string())
    );
}

#[test]
fn build_browse_url_trims_trailing_slash_on_site_url() {
    assert_eq!(
        build_browse_url(Some("https://mysite.atlassian.net/"), "KAN-1"),
        Some("https://mysite.atlassian.net/browse/KAN-1".to_string())
    );
}

#[test]
fn build_browse_url_returns_none_when_site_url_is_none() {
    assert_eq!(build_browse_url(None, "KAN-1"), None);
}

#[test]
fn build_browse_url_returns_none_when_site_url_is_only_a_slash() {
    assert_eq!(build_browse_url(Some("/"), "KAN-1"), None);
}

#[test]
fn build_browse_url_returns_none_when_site_url_is_empty_string() {
    assert_eq!(build_browse_url(Some(""), "KAN-1"), None);
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

#[test]
fn expand_mentions_leaves_text_only_tree_unchanged() {
    let mut content = vec![json!({
        "type": "paragraph",
        "content": [{"type": "text", "text": "no mentions here"}]
    })];
    let expected = content.clone();
    expand_mentions_in_content(&mut content, &mut |id| Ok(format!("@{id}"))).expect("should succeed");
    assert_eq!(content, expected);
}

#[test]
fn expand_mentions_replaces_a_mention_only_node() {
    let mut content = vec![json!({
        "type": "paragraph",
        "content": [{"type": "text", "text": "{{mention:5b10ac8d}}"}]
    })];
    expand_mentions_in_content(&mut content, &mut |id| Ok(format!("Display {id}"))).expect("should succeed");
    assert_eq!(
        content,
        vec![json!({
            "type": "paragraph",
            "content": [{
                "type": "mention",
                "attrs": {"id": "5b10ac8d", "text": "@Display 5b10ac8d"}
            }]
        })]
    );
}

#[test]
fn expand_mentions_splits_mid_string_mention_preserving_marks() {
    let mut content = vec![json!({
        "type": "paragraph",
        "content": [{
            "type": "text",
            "text": "Thanks {{mention:5b10ac8d}} for the fix",
            "marks": [{"type": "strong"}]
        }]
    })];
    expand_mentions_in_content(&mut content, &mut |id| Ok(format!("@{id}"))).expect("should succeed");
    assert_eq!(
        content,
        vec![json!({
            "type": "paragraph",
            "content": [
                {"type": "text", "text": "Thanks ", "marks": [{"type": "strong"}]},
                {"type": "mention", "attrs": {"id": "5b10ac8d", "text": "@@5b10ac8d"}},
                {"type": "text", "text": " for the fix", "marks": [{"type": "strong"}]}
            ]
        })]
    );
}

#[test]
fn expand_mentions_recurses_into_bullet_list_items() {
    let mut content = vec![json!({
        "type": "bulletList",
        "content": [{
            "type": "listItem",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "ping {{mention:aaa}}"}]
            }]
        }]
    })];
    expand_mentions_in_content(&mut content, &mut |id| Ok(format!("@{id}"))).expect("should succeed");
    assert_eq!(
        content,
        vec![json!({
            "type": "bulletList",
            "content": [{
                "type": "listItem",
                "content": [{
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "ping "},
                        {"type": "mention", "attrs": {"id": "aaa", "text": "@@aaa"}}
                    ]
                }]
            }]
        })]
    );
}

#[test]
fn expand_mentions_recurses_into_headings() {
    let mut content = vec![json!({
        "type": "heading",
        "attrs": {"level": 2},
        "content": [{"type": "text", "text": "{{mention:bbb}} review"}]
    })];
    expand_mentions_in_content(&mut content, &mut |id| Ok(format!("@{id}"))).expect("should succeed");
    assert_eq!(
        content,
        vec![json!({
            "type": "heading",
            "attrs": {"level": 2},
            "content": [
                {"type": "mention", "attrs": {"id": "bbb", "text": "@@bbb"}},
                {"type": "text", "text": " review"}
            ]
        })]
    );
}

#[test]
fn expand_mentions_handles_multiple_mentions_in_one_node() {
    let mut content = vec![json!({
        "type": "paragraph",
        "content": [{"type": "text", "text": "hi {{mention:aaa}} and {{mention:bbb}} bye"}]
    })];
    expand_mentions_in_content(&mut content, &mut |id| Ok(format!("@{id}"))).expect("should succeed");
    assert_eq!(
        content,
        vec![json!({
            "type": "paragraph",
            "content": [
                {"type": "text", "text": "hi "},
                {"type": "mention", "attrs": {"id": "aaa", "text": "@@aaa"}},
                {"type": "text", "text": " and "},
                {"type": "mention", "attrs": {"id": "bbb", "text": "@@bbb"}},
                {"type": "text", "text": " bye"}
            ]
        })]
    );
}

#[test]
fn expand_mentions_propagates_resolver_error() {
    let mut content = vec![json!({
        "type": "paragraph",
        "content": [{"type": "text", "text": "{{mention:unknown}}"}]
    })];
    let result = expand_mentions_in_content(&mut content, &mut |_id| {
        Err(crate::client::ClientError::Request("lookup failed".to_string()))
    });
    assert!(result.is_err());
}
