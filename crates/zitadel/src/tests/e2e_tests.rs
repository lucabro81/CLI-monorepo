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
//! Mostly read-only: tests use the logged-in identity itself as the known
//! fixture (its user id, username and organization from `GET /auth/v1/users/me`).
//! The IdP-link test (issue #230) creates its own fixtures — a human user and a
//! generic OIDC identity provider that is never used to log in, both named
//! `zitadel-cli-e2e-<timestamp>` — removed on drop (`LinkedHuman`), and
//! `e2e_cleanup` removes leftovers of an interrupted run.

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
    let credentials = auth::load_credentials(&config, &auth::credentials_path(&config_dir, &auth::Identity::Service), &auth::Identity::Service)
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

    let (report, all_ok) = doctor::run_doctor_in(&config_dir, &auth::Identity::Service);

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
        email_exact: None,
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
        email_exact: None,
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

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_user_authorizations_lists_only_that_user() {
    // Issue #229. The identity's own list may be empty (a service user usually
    // holds manager roles, not project authorizations): assert the shape, and
    // that every result, if any, belongs to it.
    let (client, me) = setup();
    let body = user::build_authorizations_body(&me.user_id, None, None, 100, 0);

    let result = client.list_authorizations(&body).unwrap();

    assert!(result["pagination"].is_object(), "got {result:#}");
    for authorization in result["authorizations"].as_array().map(Vec::as_slice).unwrap_or_default() {
        assert_eq!(authorization["user"]["id"], me.user_id.as_str());
        assert!(authorization["project"]["id"].is_string());
        assert!(authorization["state"].is_string());
    }
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_list_authorizations_cannot_tell_an_unknown_user_apart() {
    // Why `user authorizations` calls `get_user` first (issue #229): the list
    // answers an unknown user exactly like a user with no roles.
    let (client, _) = setup();
    let body = user::build_authorizations_body("999999999999999999", None, None, 100, 0);

    let result = client.list_authorizations(&body).unwrap();

    assert!(result.get("authorizations").is_none(), "got {result:#}");
    assert!(matches!(
        client.get_user("999999999999999999").unwrap_err(),
        crate::client::ClientError::Status { status: 404, .. }
    ));
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_user_idp_links_of_the_identity_parse() {
    // Issue #230. A service user usually has no links: assert the shape only.
    let (client, me) = setup();

    let result = client.list_idp_links(&me.user_id, &user::build_idp_links_body(100, 0)).unwrap();

    assert!(result["details"].is_object(), "got {result:#}");
    for link in result["result"].as_array().map(Vec::as_slice).unwrap_or_default() {
        assert!(link["idpId"].is_string() && link["userId"].is_string(), "got {link:#}");
    }
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_user_search_email_exact_does_not_match_a_fragment() {
    // Issue #230: --email-exact is a whole-address match, unlike --email.
    let (client, _) = setup();
    let body = user::build_search_body(&user::UserSearchFilters {
        email: None,
        email_exact: Some("@"),
        username: None,
        state: None,
        organization_id: None,
        limit: 100,
        offset: 0,
    });

    let result = client.search_users(&body).unwrap();

    assert!(result.get("result").is_none(), "a bare \"@\" must match no address, got {result:#}");
}

// ── fixtures created by the test itself (issue #230) ──────────────────────

const E2E_PREFIX: &str = "zitadel-cli-e2e-";
/// The external account id the fixture link carries (a made-up Google-like `sub`).
const EXTERNAL_USER_ID: &str = "108123456789012345678";

/// A human user linked to a throwaway generic OIDC identity provider; both are
/// deleted on drop.
struct LinkedHuman<'a> {
    client: &'a ZitadelClient,
    user_id: String,
    email: String,
    idp_id: String,
    idp_name: String,
}

impl<'a> LinkedHuman<'a> {
    fn create(client: &'a ZitadelClient) -> Self {
        use reqwest::Method;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        let idp_name = format!("{E2E_PREFIX}idp-{stamp}");
        let email = format!("{E2E_PREFIX}{stamp}@example.com");
        let idp = client
            .send_for_tests(&Method::POST, "/management/v1/idps/generic_oidc", Some(&serde_json::json!({
                "name": idp_name, "issuer": "https://accounts.google.com", "clientId": "e2e", "clientSecret": "e2e",
                "scopes": ["openid"],
                "providerOptions": {"isLinkingAllowed": true, "isCreationAllowed": false, "isAutoCreation": false, "isAutoUpdate": false},
            })))
            .unwrap();
        let idp_id = idp["id"].as_str().unwrap().to_string();
        let user = client
            .send_for_tests(&Method::POST, "/v2/users/human", Some(&serde_json::json!({
                "username": format!("{E2E_PREFIX}human-{stamp}"),
                "profile": {"givenName": "E2E", "familyName": "Human"},
                "email": {"email": email, "isVerified": true},
            })))
            .unwrap();
        let fixture = LinkedHuman { client, user_id: user["userId"].as_str().unwrap().to_string(), email, idp_id, idp_name };
        client
            .send_for_tests(&Method::POST, &format!("/v2/users/{}/links", fixture.user_id), Some(&serde_json::json!({
                "idpLink": {"idpId": fixture.idp_id, "userId": EXTERNAL_USER_ID, "userName": fixture.email},
            })))
            .unwrap();
        fixture
    }
}

impl Drop for LinkedHuman<'_> {
    fn drop(&mut self) {
        let _ = self.client.send_for_tests(&reqwest::Method::DELETE, &format!("/management/v1/users/{}", self.user_id), None);
        let _ = self.client.send_for_tests(&reqwest::Method::DELETE, &format!("/management/v1/idps/templates/{}", self.idp_id), None);
    }
}

fn exact_email_search(email: &str) -> Value {
    user::build_search_body(&user::UserSearchFilters {
        email: None,
        email_exact: Some(email),
        username: None,
        state: None,
        organization_id: None,
        limit: 100,
        offset: 0,
    })
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_a_linked_human_is_found_by_exact_email_and_its_links_name_the_provider() {
    // Issue #230: the reads Mercury chains — exact verified email, then the link.
    let (client, _) = setup();
    let fixture = LinkedHuman::create(&client);

    let found = client.search_users(&exact_email_search(&fixture.email.to_uppercase())).unwrap();
    assert_eq!(found["result"].as_array().unwrap().len(), 1, "got {found:#}");
    assert_eq!(found["result"][0]["userId"], fixture.user_id.as_str());
    assert_eq!(found["result"][0]["human"]["email"]["isVerified"], true);

    let local_part = &fixture.email[..fixture.email.find('@').unwrap()];
    let fragment = client.search_users(&exact_email_search(local_part)).unwrap();
    assert!(fragment.get("result").is_none(), "a fragment must not match, got {fragment:#}");

    let links = client.list_idp_links(&fixture.user_id, &user::build_idp_links_body(100, 0)).unwrap();
    assert_eq!(links["result"].as_array().unwrap().len(), 1, "got {links:#}");
    assert_eq!(links["result"][0]["idpId"], fixture.idp_id.as_str());
    assert_eq!(links["result"][0]["userId"], EXTERNAL_USER_ID);
    assert_eq!(links["result"][0]["userName"], fixture.email.as_str());

    let idp = client.get_idp(&fixture.idp_id).unwrap();
    assert_eq!(idp["idp"]["name"], fixture.idp_name.as_str());
}

#[test]
#[ignore = "e2e: requires zitadel init"]
fn e2e_cleanup() {
    // Removes fixtures left behind by an interrupted run.
    use reqwest::Method;
    let (client, _) = setup();
    let users = client
        .search_users(&serde_json::json!({"queries": [{"userNameQuery": {"userName": E2E_PREFIX, "method": "TEXT_QUERY_METHOD_STARTS_WITH"}}]}))
        .unwrap();
    for leftover in users["result"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let id = leftover["userId"].as_str().unwrap();
        client.send_for_tests(&Method::DELETE, &format!("/management/v1/users/{id}"), None).unwrap();
    }
    let idps = client.send_for_tests(&Method::POST, "/management/v1/idps/templates/_search", Some(&serde_json::json!({}))).unwrap();
    for leftover in idps["result"].as_array().map(Vec::as_slice).unwrap_or_default() {
        if leftover["name"].as_str().is_some_and(|name| name.starts_with(E2E_PREFIX)) {
            let id = leftover["id"].as_str().unwrap();
            client.send_for_tests(&Method::DELETE, &format!("/management/v1/idps/templates/{id}"), None).unwrap();
        }
    }
}
