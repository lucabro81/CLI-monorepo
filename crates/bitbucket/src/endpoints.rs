//! Centralized URL and path constants for the Bitbucket OAuth and REST API v2.0
//! endpoints used by [`crate::auth`] and [`crate::client`].

// ── Bitbucket OAuth (auth.rs) ──────────────────────────────────────────────

/// Token endpoint for the `client_credentials` grant. Authenticated with HTTP
/// Basic auth using the OAuth consumer's `client_id`/`client_secret`.
pub const BITBUCKET_TOKEN_URL: &str = "https://bitbucket.org/site/oauth2/access_token";

// ── Bitbucket REST API v2.0 (client.rs) ────────────────────────────────────

/// Base URL for Bitbucket REST API v2.0 calls.
pub const BITBUCKET_API_BASE_URL: &str = "https://api.bitbucket.org/2.0";

/// The authenticated user's account.
pub const PATH_USER: &str = "/user";

/// A single repository, identified by workspace slug and repo slug.
pub fn path_repository(workspace: &str, repo_slug: &str) -> String {
    format!("/repositories/{workspace}/{repo_slug}")
}

/// Repositories within a workspace, optionally paginated.
pub fn path_repositories(workspace: &str, page: Option<u32>) -> String {
    match page {
        Some(page) => format!("/repositories/{workspace}?page={page}"),
        None => format!("/repositories/{workspace}"),
    }
}
