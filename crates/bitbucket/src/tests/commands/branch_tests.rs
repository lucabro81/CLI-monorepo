#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_branch_name, infer_kind, resolve_prefix_from_branching_model, slugify};

#[test]
fn infer_kind_maps_bug_to_bugfix() {
    assert_eq!(infer_kind("Bug"), "bugfix");
}

#[test]
fn infer_kind_is_case_insensitive() {
    assert_eq!(infer_kind("bug"), "bugfix");
    assert_eq!(infer_kind("BUG"), "bugfix");
}

#[test]
fn infer_kind_maps_other_types_to_feature() {
    assert_eq!(infer_kind("Task"), "feature");
    assert_eq!(infer_kind("Story"), "feature");
    assert_eq!(infer_kind("Epic"), "feature");
}

#[test]
fn resolve_prefix_from_branching_model_returns_enabled_desired_kind() {
    let model = serde_json::json!({"branch_types": [
        {"kind": "bugfix", "enabled": true, "prefix": "bugfix/"},
        {"kind": "feature", "enabled": true, "prefix": "feature/"},
    ]});

    assert_eq!(
        resolve_prefix_from_branching_model(&model, "bugfix"),
        Some("bugfix".to_string())
    );
}

#[test]
fn resolve_prefix_from_branching_model_falls_back_to_feature_when_desired_kind_disabled() {
    let model = serde_json::json!({"branch_types": [
        {"kind": "bugfix", "enabled": false, "prefix": "bugfix/"},
        {"kind": "feature", "enabled": true, "prefix": "feature/"},
    ]});

    assert_eq!(
        resolve_prefix_from_branching_model(&model, "bugfix"),
        Some("feature".to_string())
    );
}

#[test]
fn resolve_prefix_from_branching_model_falls_back_to_feature_when_desired_kind_missing() {
    let model = serde_json::json!({"branch_types": [
        {"kind": "feature", "enabled": true, "prefix": "feature/"},
    ]});

    assert_eq!(
        resolve_prefix_from_branching_model(&model, "bugfix"),
        Some("feature".to_string())
    );
}

#[test]
fn resolve_prefix_from_branching_model_returns_none_when_neither_available() {
    let model = serde_json::json!({"branch_types": [
        {"kind": "bugfix", "enabled": false, "prefix": "bugfix/"},
        {"kind": "feature", "enabled": false, "prefix": "feature/"},
    ]});

    assert_eq!(resolve_prefix_from_branching_model(&model, "bugfix"), None);
}

#[test]
fn slugify_lowercases_and_hyphenates() {
    assert_eq!(slugify("Fix crash on startup"), "fix-crash-on-startup");
}

#[test]
fn slugify_collapses_consecutive_separators_and_truncates_at_max_len() {
    // Full slug before truncation would be 83 chars:
    // "costruire-griglia-smartlocker-v2-a-partire-da-config-hardware-logica-clavisgettings"
    // MAX_SLUG_LEN = 60 cuts mid-word inside "hardware", landing on a
    // non-hyphen char, so no extra trailing-hyphen trim is needed here.
    assert_eq!(
        slugify("Costruire griglia Smartlocker v2 a partire da config hardware/logica (clavisGetSettings)"),
        "costruire-griglia-smartlocker-v2-a-partire-da-config-hardwar"
    );
}

#[test]
fn slugify_returns_empty_string_for_all_punctuation() {
    assert_eq!(slugify("!!! ??? ..."), "");
}

#[test]
fn slugify_truncates_to_max_len_without_trailing_hyphen() {
    let long_summary = "word ".repeat(30); // far longer than MAX_SLUG_LEN
    let slug = slugify(&long_summary);

    assert!(slug.len() <= 60);
    assert!(!slug.ends_with('-'));
}

#[test]
fn build_branch_name_combines_prefix_key_and_slug() {
    assert_eq!(
        build_branch_name("feature", "SBF-19", "my-fix"),
        "feature/SBF-19-my-fix"
    );
}

#[test]
fn build_branch_name_omits_prefix_segment_when_empty() {
    assert_eq!(build_branch_name("", "SBF-19", "my-fix"), "SBF-19-my-fix");
}

#[test]
fn build_branch_name_omits_trailing_hyphen_when_slug_empty() {
    assert_eq!(build_branch_name("feature", "SBF-19", ""), "feature/SBF-19");
}

#[test]
fn build_branch_name_omits_both_when_prefix_and_slug_empty() {
    assert_eq!(build_branch_name("", "SBF-19", ""), "SBF-19");
}
