//! End-to-end tests against a real ZITADEL instance.
//!
//! # Prerequisites
//!
//! - `zitadel init --instance-url ... --key-file ...` has been run for the config
//!   dir in use (`$XDG_CONFIG_HOME/zitadel-cli/`), with a service user holding at
//!   least `ORG_OWNER` on its own organization (`zitadel doctor` passes).
//!
//! # Running
//!
//! ```sh
//! cargo test -p zitadel -- --ignored
//! # against a scratch config dir:
//! XDG_CONFIG_HOME=/path/to/scratch cargo test -p zitadel -- --ignored
//! ```
//!
//! # Isolation
//!
//! Read-only: every test uses the logged-in identity itself as the known
//! fixture (its user id, username and organization from `GET /auth/v1/users/me`),
//! so nothing is created and nothing needs cleaning up.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;

use crate::auth::{self, AppConfig};
use crate::client::ZitadelClient;
use crate::commands::{doctor, organization, project, user};
use crate::context;

struct Me {
    user_id: String,
    user_name: String,
    organization_id: String,
}

fn setup() -> (ZitadelClient, Me) {
    let config_dir = context::config_dir().expect("could not resolve config dir");
    let config = AppConfig::load(&auth::app_config_path(&config_dir))
        .expect("app.json not found — run `zitadel init` first");
    let credentials = auth::load_credentials(&config, &auth::credentials_path(&config_dir, auth::Identity::Service))
        .expect("not authenticated — run `zitadel auth login` first");
    let client = ZitadelClient::new(&config.instance_url, &credentials);
    let me = client.get_current_user().expect("GET /auth/v1/users/me failed");
    let me = Me {
        user_id: me["user"]["id"].as_str().unwrap().to_string(),
        user_name: me["user"]["userName"].as_str().unwrap().to_string(),
        organization_id: me["user"]["details"]["resourceOwner"].as_str().unwrap().to_string(),
    };
    (client, me)
}

fn ids(list: &Value, key: &str) -> Vec<String> {
    list.as_array()
        .map(|items| items.iter().filter_map(|i| i[key].as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_doctor_passes_for_the_configured_identity() {
    let config_dir = context::config_dir().unwrap();

    let (report, all_ok) = doctor::run_doctor_in(&config_dir, auth::Identity::Service);

    assert!(all_ok, "doctor report: {report:#}");
    assert!(!report["memberships"]["memberships"].as_array().unwrap().is_empty());
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_user_get_returns_the_logged_in_identity() {
    let (client, me) = setup();

    let user = client.get_user(&me.user_id).unwrap();

    assert_eq!(user["user"]["userId"], me.user_id.as_str());
    assert_eq!(user["user"]["username"], me.user_name.as_str());
    assert_eq!(user["user"]["details"]["resourceOwner"], me.organization_id.as_str());
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_user_get_unknown_id_is_404() {
    let (client, _) = setup();

    let err = client.get_user("999999999999999999").unwrap_err();

    assert!(
        matches!(err, crate::client::ClientError::Status { status: 404, .. }),
        "got {err:?}"
    );
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_user_search_by_username_and_organization_finds_the_identity() {
    let (client, me) = setup();
    let body = user::build_search_body(&user::UserSearchFilters {
        email: None,
        username: Some(&me.user_name),
        state: Some(crate::cli::UserState::Active),
        organization_id: Some(&me.organization_id),
        limit: 100,
        offset: 0,
    });

    let result = client.search_users(&body).unwrap();

    assert!(ids(&result["result"], "userId").contains(&me.user_id), "got {result:#}");
    assert!(result["details"]["totalResult"].as_str().unwrap().parse::<u64>().unwrap() >= 1);
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_user_search_respects_limit() {
    let (client, _) = setup();
    let body = user::build_search_body(&user::UserSearchFilters {
        email: None,
        username: None,
        state: None,
        organization_id: None,
        limit: 1,
        offset: 0,
    });

    let result = client.search_users(&body).unwrap();

    assert_eq!(result["result"].as_array().unwrap().len(), 1);
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_organization_list_includes_the_identity_organization() {
    let (client, me) = setup();

    let result = client.list_organizations(&organization::build_list_body(None, 100, 0)).unwrap();

    assert!(ids(&result["result"], "id").contains(&me.organization_id), "got {result:#}");
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_project_list_filters_by_organization() {
    let (client, me) = setup();

    let own = client
        .list_projects(&project::build_list_body(None, Some(&me.organization_id), 100, 0))
        .unwrap();
    let none = client
        .list_projects(&project::build_list_body(None, Some("111111111111111111"), 100, 0))
        .unwrap();

    assert!(own.get("pagination").is_some(), "got {own:#}");
    for project in own["projects"].as_array().into_iter().flatten() {
        assert_eq!(project["organizationId"], me.organization_id.as_str());
    }
    assert!(none.get("projects").is_none_or(|p| p.as_array().unwrap().is_empty()), "got {none:#}");
}
