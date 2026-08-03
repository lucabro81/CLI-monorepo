//! URL constants for the Atlassian identity platform (`auth.atlassian.com` /
//! `api.atlassian.com`). Identical across every Atlassian Cloud product —
//! Jira, Confluence, etc. all authenticate against the same OAuth 2.0
//! endpoints and resolve `cloud_id` via the same accessible-resources call.
//! Product-specific REST API base URLs (e.g. `api.atlassian.com/ex/jira`)
//! stay in each crate's own `endpoints.rs`.

/// `audience` parameter required by both the `client_credentials` and
/// `authorization_code` token requests.
pub const ATLASSIAN_AUDIENCE: &str = "api.atlassian.com";

/// Token endpoint for both OAuth grant types.
pub const ATLASSIAN_TOKEN_URL: &str = "https://auth.atlassian.com/oauth/token";

/// Authorization endpoint for the 3LO + PKCE flow.
pub const ATLASSIAN_AUTHORIZE_URL: &str = "https://auth.atlassian.com/authorize";

/// Resolves the `cloud_id`(s) of the Atlassian site(s) accessible with a given access token.
pub const ATLASSIAN_ACCESSIBLE_RESOURCES_URL: &str =
    "https://api.atlassian.com/oauth/token/accessible-resources";
