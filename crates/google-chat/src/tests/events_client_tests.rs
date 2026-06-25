#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::build_subscription_body;

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
