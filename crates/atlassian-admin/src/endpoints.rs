//! Centralized URL and path constants for the Atlassian Organization Admin
//! API used by [`crate::client`].
//!
//! Two distinct hosts/scopes are involved, confirmed live (2026-07-22)
//! against a real organization after the initially documented path for
//! user lookup turned out wrong (404 "failed to match any route"):
//! - The **Organization API** (`.../admin/v1/orgs/{org_id}`) — requires the
//!   `read:orgs:admin` scope. Used only by `doctor`'s live sanity check.
//! - The **user management "manage" API** (`api.atlassian.com/users/{account_id}/manage/profile`,
//!   no `/admin` prefix, no `org_id` in the path at all — the org is implied
//!   by the API key itself). `.../manage` alone (no `/profile` suffix) is a
//!   *capabilities* endpoint (which actions are allowed on this user —
//!   apiToken.create, email.set, etc. — not the profile itself);
//!   `.../manage/profile` is the actual profile resource, including `email`.
//!   Both are gated by a `manage:org` scope that does **not** appear in
//!   Atlassian's public scope catalog (developer.atlassian.com/cloud/admin/scopes)
//!   and so cannot be selected when creating a scoped ("with scopes")
//!   Organization API key — confirmed only reachable with an unscoped
//!   ("without scopes") key. Used by `user get`.

/// Base URL for the Atlassian Organization API (`/admin/v1/orgs/...`).
pub const ORG_ADMIN_API_BASE_URL: &str = "https://api.atlassian.com/admin";

/// Base URL for the Atlassian user management "manage" API (no `/admin` prefix).
pub const USER_MANAGEMENT_API_BASE_URL: &str = "https://api.atlassian.com";

/// A single organization, identified by its org id. Used by `doctor` as a
/// lightweight live check — cheaper than a full user lookup. Requires the
/// `read:orgs:admin` scope.
pub fn path_organization(org_id: &str) -> String {
    format!("{ORG_ADMIN_API_BASE_URL}/v1/orgs/{org_id}")
}

/// A managed user's profile (including email), identified by their Atlassian
/// `account_id`. No `org_id` in the path — the organization is implied by
/// the API key itself. Requires an unscoped ("without scopes") Organization
/// API key — see this module's doc comment.
pub fn path_user_manage_profile(account_id: &str) -> String {
    format!("{USER_MANAGEMENT_API_BASE_URL}/users/{account_id}/manage/profile")
}

/// All managed users in an organization (paginated via an opaque `cursor`
/// from the previous response's `links.next`), each entry already including
/// `account_id`/`name`/`email` directly — no per-user follow-up call needed.
/// Documented (not yet independently confirmed live, single-page org tested)
/// to require the `read:accounts:admin` scope, unlike `user get`. Returns the
/// bare path with no query string — `client.rs::list_users` appends the
/// `cursor` param itself (properly URL-encoded, since it's an opaque,
/// possibly base64-shaped token, same treatment as jira's `page_token`).
pub fn path_list_users(org_id: &str) -> String {
    format!("{ORG_ADMIN_API_BASE_URL}/v1/orgs/{org_id}/users")
}
