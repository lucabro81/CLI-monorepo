//! Centralized URL and path constants for the Bitbucket OAuth and REST API v2.0
//! endpoints used by [`crate::auth`] and [`crate::client`].

// ── Bitbucket OAuth (auth.rs) ──────────────────────────────────────────────

/// Token endpoint for every grant (`client_credentials`, `authorization_code`,
/// `refresh_token`). Authenticated with HTTP Basic auth using the OAuth
/// consumer's `client_id`/`client_secret`.
pub const BITBUCKET_TOKEN_URL: &str = "https://bitbucket.org/site/oauth2/access_token";

/// Authorization endpoint for the `authorization_code` grant (`auth login --user`).
/// Bitbucket redirects back to the consumer's configured callback URL — there
/// is no `redirect_uri` parameter.
pub const BITBUCKET_AUTHORIZE_URL: &str = "https://bitbucket.org/site/oauth2/authorize";

/// Local address `auth login --user` listens on for the authorization callback.
/// The OAuth consumer's callback URL must point here: `http://localhost:8080/callback`.
pub const CALLBACK_LISTEN_ADDRESS: &str = "127.0.0.1:8080";
/// Path of the consumer's callback URL (`http://localhost:8080/callback`).
pub const CALLBACK_PATH: &str = "/callback";

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

/// Comments on a single pull request, identified by its numeric ID, optionally paginated.
pub fn path_pull_request_comments(workspace: &str, repo_slug: &str, id: u64, page: Option<u32>) -> String {
    match page {
        Some(page) => format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments?page={page}"),
        None => format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments"),
    }
}

/// A single comment on a pull request. `PUT` to update its text.
pub fn path_pull_request_comment(workspace: &str, repo_slug: &str, id: u64, comment_id: u64) -> String {
    format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments/{comment_id}")
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

/// Members of a workspace, optionally paginated.
pub fn path_workspace_members(workspace: &str, page: Option<u32>) -> String {
    match page {
        Some(page) => format!("/workspaces/{workspace}/members?page={page}"),
        None => format!("/workspaces/{workspace}/members"),
    }
}

/// Branches within a repository, optionally paginated.
pub fn path_branches(workspace: &str, repo_slug: &str, page: Option<u32>) -> String {
    match page {
        Some(page) => format!("/repositories/{workspace}/{repo_slug}/refs/branches?page={page}"),
        None => format!("/repositories/{workspace}/{repo_slug}/refs/branches"),
    }
}

/// The effective branching model configured for a repository.
pub fn path_branching_model(workspace: &str, repo_slug: &str) -> String {
    format!("/repositories/{workspace}/{repo_slug}/branching-model")
}

/// The raw unified diff for a pull request, optionally with extra context lines
/// around each change and/or restricted to a single file path.
pub fn path_pull_request_diff(workspace: &str, repo_slug: &str, id: u64, context: Option<u32>, path: Option<&str>) -> String {
    let mut params = Vec::new();
    if let Some(context) = context {
        params.push(format!("context={context}"));
    }
    if let Some(path) = path {
        params.push(format!("path={path}"));
    }

    let base = format!("/repositories/{workspace}/{repo_slug}/pullrequests/{id}/diff");
    if params.is_empty() {
        base
    } else {
        format!("{base}?{}", params.join("&"))
    }
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
