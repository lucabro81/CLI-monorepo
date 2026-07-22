//! Centralized URL and path constants for the Atlassian Organization Admin
//! API used by [`crate::client`].

/// Base URL for the Atlassian Organization Admin API.
pub const ADMIN_API_BASE_URL: &str = "https://api.atlassian.com/admin";

/// A single organization, identified by its org id. Used by `doctor` as a
/// lightweight live check — cheaper than a full user lookup.
pub fn path_organization(org_id: &str) -> String {
    format!("/v1/orgs/{org_id}")
}

/// A managed user's profile (including email) within an organization,
/// identified by their Atlassian `account_id`.
pub fn path_user_manage(org_id: &str, account_id: &str) -> String {
    format!("/v1/orgs/{org_id}/users/{account_id}/manage")
}
