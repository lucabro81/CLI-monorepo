//! Centralized URL and path constants for the Google OAuth and Google Chat API v1
//! endpoints used by [`crate::auth`] and [`crate::client`]. Keeping these in one
//! place avoids subtly inconsistent hardcoded strings spread across both modules.

// ── Google OAuth 2.0 (auth.rs) ─────────────────────────────────────────────

/// Authorization endpoint for the OAuth 2.0 Authorization Code + PKCE flow.
pub const GOOGLE_OAUTH_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

/// Token endpoint for the authorization-code exchange, refresh-token exchange,
/// and the JWT-bearer (domain-wide delegation) exchange. Always called with
/// `application/x-www-form-urlencoded` — Google's token endpoint rejects a
/// JSON body for the `jwt-bearer` grant with `unsupported_grant_type`.
pub const GOOGLE_OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// `grant_type` value for the service-account domain-wide-delegation flow (RFC 7523).
pub const JWT_BEARER_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";

// ── Google Chat API v1 (client.rs) ─────────────────────────────────────────

/// Base URL for Google Chat API v1 calls.
pub const CHAT_API_BASE_URL: &str = "https://chat.googleapis.com/v1";

pub const PATH_SPACES: &str = "/spaces";
