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
