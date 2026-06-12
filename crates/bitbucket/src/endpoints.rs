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

/// A single pull request, identified by its numeric ID.
pub fn path_pull_request(workspace: &str, repo_slug: &str, id: u64) -> String {
    format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}")
}

/// Comments on a single pull request, identified by its numeric ID.
pub fn path_pull_request_comments(workspace: &str, repo_slug: &str, id: u64) -> String {
    format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments")
}

/// The current user's approval of a pull request. `POST` to approve, `DELETE` to unapprove.
pub fn path_pull_request_approve(workspace: &str, repo_slug: &str, id: u64) -> String {
    format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}/approve")
}

/// Declines a pull request. `POST` only.
pub fn path_pull_request_decline(workspace: &str, repo_slug: &str, id: u64) -> String {
    format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}/decline")
}

/// Merges a pull request. `POST` only.
pub fn path_pull_request_merge(workspace: &str, repo_slug: &str, id: u64) -> String {
    format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}/merge")
}

/// Pull requests for a repository, optionally filtered by `state`
/// (`OPEN`, `MERGED`, `DECLINED`, `SUPERSEDED`) and paginated.
pub fn path_pull_requests(workspace: &str, repo_slug: &str, state: Option<&str>, page: Option<u32>) -> String {
    let mut params = Vec::new();
    if let Some(state) = state {
        params.push(format!("state={state}"));
    }
    if let Some(page) = page {
        params.push(format!("page={page}"));
    }

    let base = format!("/repositories/{workspace}/{repo_slug}/pullrequests");
    if params.is_empty() {
        base
    } else {
        format!("{base}?{}", params.join("&"))
    }
}
