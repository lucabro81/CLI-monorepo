//! Centralized URL and path constants for the Confluence Cloud REST API,
//! used by [`crate::client`]. Keeping these in one place avoids subtly
//! inconsistent hardcoded strings spread across the module. Atlassian OAuth
//! endpoints (used by `auth.rs`) live in the shared `atlassian_auth::endpoints`
//! crate instead — see root `CLAUDE.md`'s "Shared library: crates/atlassian-auth".
//!
//! Confluence Cloud exposes two REST API generations side by side under the
//! same site: v2 (`/wiki/api/v2/...`), the current actively developed
//! surface, and v1 (`/wiki/rest/api/...`) for the handful of things v2
//! doesn't cover (current-user identity, CQL search, templates). See this
//! crate's CLAUDE.md "API design notes" for why each command picks the
//! version it does.

/// Base URL for Confluence Cloud REST API calls; the client appends
/// `/<cloud_id>` to this, then a `/wiki/...` path.
pub const CONFLUENCE_API_BASE_URL: &str = "https://api.atlassian.com/ex/confluence";

/// Current-user identity check (`auth whoami`). v1 — no v2 equivalent exists.
pub const PATH_USER_CURRENT: &str = "/wiki/rest/api/user/current";

/// CQL content search (`page search`). v1 — no v2 equivalent exists yet.
pub const PATH_CONTENT_SEARCH: &str = "/wiki/rest/api/content/search";

/// Template lookup, used to resolve `page create --template-id`. v1 — no v2 equivalent exists.
pub const PATH_TEMPLATE: &str = "/wiki/rest/api/template";

pub const PATH_PAGES: &str = "/wiki/api/v2/pages";
pub const PATH_SPACES: &str = "/wiki/api/v2/spaces";

/// `/wiki/api/v2/pages/<id>`
pub fn page_path(id: &str) -> String {
    format!("{PATH_PAGES}/{id}")
}

/// `/wiki/rest/api/template/<id>`
pub fn template_path(id: &str) -> String {
    format!("{PATH_TEMPLATE}/{id}")
}
