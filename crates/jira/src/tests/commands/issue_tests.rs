#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::apply_stale_filter;

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
