#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::require_message_filter;
use crate::error::CliError;

#[test]
fn require_message_filter_ok_when_filter_given() {
    let result = require_message_filter(
        Some("hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")"),
        false,
        "projects/p/subscriptions/s",
    );

    assert!(result.is_ok());
}

#[test]
fn require_message_filter_ok_when_allow_unfiltered() {
    let result = require_message_filter(None, true, "projects/p/subscriptions/s");

    assert!(result.is_ok());
}

#[test]
fn require_message_filter_errors_when_neither_given() {
    let result = require_message_filter(None, false, "projects/p/subscriptions/s");

    assert!(matches!(
        result,
        Err(CliError::MessageFilterRequired { ref pubsub_subscription })
            if pubsub_subscription == "projects/p/subscriptions/s"
    ));
}
