//! Centralized path constants and URL builders for the ZITADEL OAuth/OIDC and
//! API endpoints used by [`crate::auth`] and [`crate::client`]. Every URL is
//! relative to the configured instance URL (`app.json`'s `instance_url`, already
//! normalized without a trailing slash), since ZITADEL can be Cloud or self-hosted.

// ── OAuth 2.0 / OIDC (auth.rs) ─────────────────────────────────────────────

/// Token endpoint for the JWT-bearer (service user), authorization-code and
/// refresh-token exchanges. Called with `application/x-www-form-urlencoded`.
pub const TOKEN_PATH: &str = "/oauth/v2/token";

/// `grant_type` for the service-user private key JWT flow (RFC 7523).
pub const JWT_BEARER_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";

/// Scopes requested for the service user. The `zitadel:aud` scope puts the
/// ZITADEL project in the token audience, which is what lets the token call
/// ZITADEL's own APIs.
pub const SERVICE_USER_SCOPES: &str = "openid urn:zitadel:iam:org:project:id:zitadel:aud";

/// Authorization endpoint for the human login (authorization code + PKCE).
pub const AUTHORIZE_PATH: &str = "/oauth/v2/authorize";

/// Scopes requested by `auth login --user`. `offline_access` makes ZITADEL issue
/// a refresh token (the Native app must also have refresh tokens enabled).
pub const USER_SCOPES: &str =
    "openid profile email offline_access urn:zitadel:iam:org:project:id:zitadel:aud";

/// Redirect URI registered on the Native app, and the loopback address the CLI
/// listens on for it.
pub const REDIRECT_URI: &str = "http://localhost:8080/callback";
pub const CALLBACK_PATH: &str = "/callback";
pub const CALLBACK_LISTEN_ADDR: &str = "127.0.0.1:8080";

pub fn authorize_url(instance_url: &str) -> String {
    format!("{instance_url}{AUTHORIZE_PATH}")
}

pub fn token_url(instance_url: &str) -> String {
    format!("{instance_url}{TOKEN_PATH}")
}

// ── ZITADEL APIs (client.rs) ───────────────────────────────────────────────

/// v1 Auth API: the calling identity. No v2 "me" equivalent exists.
pub const AUTH_USERS_ME_PATH: &str = "/auth/v1/users/me";

/// v1 Auth API: the calling identity's administrator memberships (roles per
/// instance / organization / project / project grant).
pub const AUTH_MY_MEMBERSHIPS_SEARCH_PATH: &str = "/auth/v1/memberships/me/_search";

/// v2 User service: `POST` = `ListUsers` (search), `GET /{userId}` = `GetUserByID`.
pub const USERS_V2_PATH: &str = "/v2/users";

/// v2 Organization service: `ListOrganizations` (search).
pub const ORGANIZATIONS_SEARCH_V2_PATH: &str = "/v2/organizations/_search";

/// v2 Project service `ListProjects`. This service has no REST (`/v2/...`)
/// mapping: it is reachable only via its Connect-protocol path, with a `POST` of a plain
/// `application/json` body (verified live; `application/connect+json` → 415).
pub const PROJECTS_LIST_V2_PATH: &str = "/zitadel.project.v2.ProjectService/ListProjects";
