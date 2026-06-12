#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::check_permissions;
use crate::auth::Credentials;

fn credentials_with_scopes(scopes: Vec<&str>) -> Credentials {
    Credentials {
        access_token: "token".to_string(),
        expires_at: u64::MAX,
        scopes: scopes.into_iter().map(str::to_string).collect(),
    }
}

#[test]
fn reports_granted_scopes_as_ok() {
    let credentials = credentials_with_scopes(vec!["repository:admin", "pullrequest:write"]);

    let report = check_permissions(&credentials);

    assert_eq!(report["status"], "ok");
    assert_eq!(report["granted_scopes"], serde_json::json!(["repository:admin", "pullrequest:write"]));
}

#[test]
fn empty_scopes_is_an_error() {
    let credentials = credentials_with_scopes(vec![]);

    let report = check_permissions(&credentials);

    assert_eq!(report["status"], "error");
    assert_eq!(report["granted_scopes"], serde_json::json!([]));
}
