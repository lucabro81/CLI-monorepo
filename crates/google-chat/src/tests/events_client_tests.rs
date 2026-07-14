#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_pubsub_subscription_body, build_subscription_body, subscription_config_mismatch};

#[test]
fn build_subscription_body_has_expected_shape() {
    let body = build_subscription_body(
        "//chat.googleapis.com/spaces/AAQA-_d58OQ",
        &["google.workspace.chat.message.v1.created".to_string()],
        "projects/p/topics/t",
    );

    assert_eq!(
        body,
        serde_json::json!({
            "targetResource": "//chat.googleapis.com/spaces/AAQA-_d58OQ",
            "eventTypes": ["google.workspace.chat.message.v1.created"],
            "notificationEndpoint": { "pubsubTopic": "projects/p/topics/t" },
            "payloadOptions": { "includeResource": true },
        })
    );
}

#[test]
fn build_subscription_body_includes_all_event_types_in_order() {
    let body = build_subscription_body(
        "//chat.googleapis.com/spaces/AAQA-_d58OQ",
        &[
            "google.workspace.chat.message.v1.created".to_string(),
            "google.workspace.chat.message.v1.updated".to_string(),
        ],
        "projects/p/topics/t",
    );

    assert_eq!(
        body["eventTypes"],
        serde_json::json!([
            "google.workspace.chat.message.v1.created",
            "google.workspace.chat.message.v1.updated",
        ])
    );
}

#[test]
fn build_pubsub_subscription_body_without_filter() {
    let body = build_pubsub_subscription_body("projects/p/topics/t", None);

    assert_eq!(
        body,
        serde_json::json!({
            "topic": "projects/p/topics/t",
        })
    );
}

#[test]
fn build_pubsub_subscription_body_with_filter() {
    let body = build_pubsub_subscription_body(
        "projects/p/topics/t",
        Some("hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")"),
    );

    assert_eq!(
        body,
        serde_json::json!({
            "topic": "projects/p/topics/t",
            "filter": "hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")",
        })
    );
}

#[test]
fn subscription_config_mismatch_matches_when_topic_equal_and_no_filter_either_side() {
    let existing = serde_json::json!({ "topic": "projects/p/topics/t" });

    assert_eq!(
        subscription_config_mismatch(&existing, "projects/p/topics/t", None),
        None
    );
}

#[test]
fn subscription_config_mismatch_matches_when_topic_and_filter_both_equal() {
    let existing = serde_json::json!({
        "topic": "projects/p/topics/t",
        "filter": "hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")",
    });

    assert_eq!(
        subscription_config_mismatch(
            &existing,
            "projects/p/topics/t",
            Some("hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")"),
        ),
        None
    );
}

#[test]
fn subscription_config_mismatch_reports_topic_difference() {
    let existing = serde_json::json!({ "topic": "projects/p/topics/other" });

    let reason = subscription_config_mismatch(&existing, "projects/p/topics/t", None)
        .expect("expected a mismatch reason");

    assert!(reason.contains("topic"), "reason should mention topic: {reason}");
    assert!(reason.contains("projects/p/topics/other"), "reason should include the existing topic: {reason}");
    assert!(reason.contains("projects/p/topics/t"), "reason should include the requested topic: {reason}");
}

#[test]
fn subscription_config_mismatch_reports_filter_present_on_existing_but_not_requested() {
    let existing = serde_json::json!({
        "topic": "projects/p/topics/t",
        "filter": "hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")",
    });

    let reason = subscription_config_mismatch(&existing, "projects/p/topics/t", None)
        .expect("expected a mismatch reason");

    assert!(reason.contains("filter"), "reason should mention filter: {reason}");
}

#[test]
fn subscription_config_mismatch_reports_filter_requested_but_not_on_existing() {
    let existing = serde_json::json!({ "topic": "projects/p/topics/t" });

    let reason = subscription_config_mismatch(
        &existing,
        "projects/p/topics/t",
        Some("hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")"),
    )
    .expect("expected a mismatch reason");

    assert!(reason.contains("filter"), "reason should mention filter: {reason}");
}

#[test]
fn subscription_config_mismatch_reports_different_filters_on_both_sides() {
    let existing = serde_json::json!({
        "topic": "projects/p/topics/t",
        "filter": "hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")",
    });

    let reason = subscription_config_mismatch(
        &existing,
        "projects/p/topics/t",
        Some("hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/OTHER\")"),
    )
    .expect("expected a mismatch reason");

    assert!(reason.contains("filter"), "reason should mention filter: {reason}");
}
