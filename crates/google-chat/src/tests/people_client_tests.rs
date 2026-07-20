#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_get_user_url, normalize_to_people_resource};

#[test]
fn normalize_to_people_resource_prefixes_bare_id() {
    assert_eq!(
        normalize_to_people_resource("108506379394699518479"),
        "people/108506379394699518479"
    );
}

#[test]
fn normalize_to_people_resource_strips_users_prefix() {
    assert_eq!(
        normalize_to_people_resource("users/108506379394699518479"),
        "people/108506379394699518479"
    );
}

#[test]
fn normalize_to_people_resource_passes_through_people_prefix() {
    assert_eq!(
        normalize_to_people_resource("people/108506379394699518479"),
        "people/108506379394699518479"
    );
}

#[test]
fn normalize_to_people_resource_does_not_double_prefix_lookalike_bare_id() {
    // A bare id that happens to start with "people" but isn't the "people/" resource
    // prefix must still get a single "people/" added, not be mistaken for already-prefixed.
    assert_eq!(normalize_to_people_resource("peoplefoo"), "people/peoplefoo");
}

#[test]
fn build_get_user_url_requests_names_and_email_addresses() {
    assert_eq!(
        build_get_user_url("people/108506379394699518479"),
        "https://people.googleapis.com/v1/people/108506379394699518479?personFields=names,emailAddresses"
    );
}
