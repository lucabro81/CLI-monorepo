#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::normalize_space_name;

#[test]
fn normalize_space_name_passes_through_full_resource_name() {
    assert_eq!(normalize_space_name("spaces/AAQA-_d58OQ"), "spaces/AAQA-_d58OQ");
}

#[test]
fn normalize_space_name_prefixes_bare_id() {
    assert_eq!(normalize_space_name("AAQA-_d58OQ"), "spaces/AAQA-_d58OQ");
}

#[test]
fn normalize_space_name_does_not_double_prefix() {
    // A bare id that happens to start with "spaces" but isn't the "spaces/" resource
    // prefix must still get a single "spaces/" added, not be mistaken for already-prefixed.
    assert_eq!(normalize_space_name("spacesfoo"), "spaces/spacesfoo");
}
