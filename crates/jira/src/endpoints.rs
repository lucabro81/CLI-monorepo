//! Centralized URL and path constants for the Atlassian OAuth and Jira REST API v3
//! endpoints used by [`crate::auth`] and [`crate::client`]. Keeping these in one
//! place avoids subtly inconsistent hardcoded strings spread across both modules.

// ── Atlassian OAuth (auth.rs) ──────────────────────────────────────────────

/// `audience` parameter required by both the `client_credentials` and
/// `authorization_code` token requests.
pub const ATLASSIAN_AUDIENCE: &str = "api.atlassian.com";

/// Token endpoint for both OAuth grant types.
pub const ATLASSIAN_TOKEN_URL: &str = "https://auth.atlassian.com/oauth/token";

/// Authorization endpoint for the 3LO + PKCE flow.
pub const ATLASSIAN_AUTHORIZE_URL: &str = "https://auth.atlassian.com/authorize";

/// Resolves the `cloud_id` of the Jira site(s) accessible with a given access token.
pub const ATLASSIAN_ACCESSIBLE_RESOURCES_URL: &str =
    "https://api.atlassian.com/oauth/token/accessible-resources";

// ── Jira REST API v3 (client.rs) ───────────────────────────────────────────

/// Base URL for Jira REST API v3 calls; the client appends `/<cloud_id>` to this.
pub const JIRA_API_BASE_URL: &str = "https://api.atlassian.com/ex/jira";

pub const PATH_MYSELF: &str = "/rest/api/3/myself";
pub const PATH_MY_PERMISSIONS: &str = "/rest/api/3/mypermissions";
pub const PATH_SEARCH_JQL: &str = "/rest/api/3/search/jql";
pub const PATH_ISSUE: &str = "/rest/api/3/issue";
pub const PATH_PROJECT_SEARCH: &str = "/rest/api/3/project/search";

/// `/rest/api/3/project/<key>/role`
pub fn project_roles_path(key: &str) -> String {
    format!("/rest/api/3/project/{key}/role")
}

/// `/rest/api/3/issue/<key>`
pub fn issue_path(key: &str) -> String {
    format!("{PATH_ISSUE}/{key}")
}

/// `/rest/api/3/issue/<key>/comment`
pub fn issue_comment_path(key: &str) -> String {
    format!("{PATH_ISSUE}/{key}/comment")
}

/// `/rest/api/3/issue/<key>/comment/<comment_id>`
pub fn issue_comment_id_path(key: &str, comment_id: &str) -> String {
    format!("{PATH_ISSUE}/{key}/comment/{comment_id}")
}

/// `/rest/api/3/issue/<key>/transitions`
pub fn issue_transitions_path(key: &str) -> String {
    format!("{PATH_ISSUE}/{key}/transitions")
}
