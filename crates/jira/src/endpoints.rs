//! Centralized URL and path constants for the Jira REST API v3 endpoints used
//! by [`crate::client`]. Keeping these in one place avoids subtly inconsistent
//! hardcoded strings spread across the module. Atlassian OAuth endpoints
//! (used by `auth.rs`) live in the shared `atlassian_auth::endpoints` crate
//! instead — see root `CLAUDE.md`'s "Shared library: crates/atlassian-auth".

// ── Jira REST API v3 (client.rs) ───────────────────────────────────────────

/// Base URL for Jira REST API v3 calls; the client appends `/<cloud_id>` to this.
pub const JIRA_API_BASE_URL: &str = "https://api.atlassian.com/ex/jira";

pub const PATH_MYSELF: &str = "/rest/api/3/myself";
pub const PATH_MY_PERMISSIONS: &str = "/rest/api/3/mypermissions";
pub const PATH_SEARCH_JQL: &str = "/rest/api/3/search/jql";
pub const PATH_ISSUE: &str = "/rest/api/3/issue";
pub const PATH_PROJECT_SEARCH: &str = "/rest/api/3/project/search";
pub const PATH_USER: &str = "/rest/api/3/user";
pub const PATH_USER_SEARCH: &str = "/rest/api/3/user/search";

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

/// `/rest/api/3/issue/<key>/assignee`
pub fn issue_assignee_path(key: &str) -> String {
    format!("{PATH_ISSUE}/{key}/assignee")
}
