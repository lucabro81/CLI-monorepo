//! Confluence Cloud REST API HTTP client.
//!
//! `ConfluenceClient` wraps a blocking `reqwest` client pre-configured with a
//! `Bearer` token and the base URL `https://api.atlassian.com/ex/confluence/<cloud_id>`.
//! All methods return raw `serde_json::Value` so callers decide how much
//! structure to impose; the `--select` flag can then filter the output
//! client-side without requiring typed response structs for every endpoint.
//!
//! Calls both Confluence API generations under the same base URL — see
//! `endpoints.rs` for which path belongs to which version and why.
//!
//! Private helpers `get_json`/`post_json`/`put_json`/`delete` handle URL
//! construction and error mapping; every request goes through `send`, which adds
//! the token, renews it once and repeats the request when Confluence answers 401
//! (issue #240: a token revoked before it expires).

use oauth_user_login::BearerToken;

use crate::auth::Credentials;
use crate::endpoints;
use crate::error::CliError;

/// Error returned by `ConfluenceClient` methods.
///
/// `Request` covers network-level failures (connection refused, timeout, etc.).
/// `Status` covers HTTP-level failures where the server responded with a non-2xx status.
#[derive(Debug)]
pub enum ClientError {
    /// Network or serialization error — no HTTP response was received.
    Request(String),
    /// The server responded but with a non-2xx status code.
    Status { status: u16, body: String },
    /// Renewing the token after a 401 failed; carries the error for the user.
    Renewal(Box<CliError>),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Request(msg) => write!(f, "request failed: {msg}"),
            ClientError::Status { status, body } => {
                write!(f, "Confluence returned status {status}: {body}")
            }
            ClientError::Renewal(error) => write!(f, "{error}"),
        }
    }
}

/// Blocking HTTP client for the Confluence Cloud REST API.
pub struct ConfluenceClient {
    base_url: String,
    token: BearerToken<CliError>,
    http: reqwest::blocking::Client,
}

impl ConfluenceClient {
    /// Builds a client from stored credentials. The base URL is derived from `cloud_id`.
    pub fn new(credentials: &Credentials) -> Self {
        Self {
            base_url: format!(
                "{}/{}",
                endpoints::CONFLUENCE_API_BASE_URL, credentials.cloud_id
            ),
            token: BearerToken::new(credentials.access_token.clone()),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// On a 401 the client calls `renewer` once with the rejected token and
    /// repeats the request with the token it returns (issue #240).
    #[must_use]
    pub fn with_renewer(mut self, renewer: oauth_user_login::Renewer<CliError>) -> Self {
        self.token = self.token.with_renewer(renewer);
        self
    }

    /// The token the next request will send (renewed after a 401).
    pub fn access_token(&self) -> String {
        self.token.current()
    }

    /// Returns the currently authenticated user as raw JSON (v1 — no v2 equivalent).
    pub fn get_current_user(&self) -> Result<serde_json::Value, ClientError> {
        self.get_json(endpoints::PATH_USER_CURRENT)
    }

    /// Fetches a page by ID, including its storage-format body, as raw JSON.
    pub fn get_page(&self, id: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(&format!("{}?body-format=storage", endpoints::page_path(id)))
    }

    /// Creates a page from a pre-built request body (`spaceId`, `title`, `body`,
    /// optional `parentId`) and returns the created page as raw JSON.
    pub fn create_page(&self, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        self.post_json(endpoints::PATH_PAGES, body)
    }

    /// Updates a page from a pre-built request body (`id`, `status`, `title`,
    /// `body`, `version`) and returns the updated page as raw JSON. Confluence's
    /// v2 API requires the full page representation on every update — there is
    /// no partial-patch endpoint — and a `version.number` exactly one greater
    /// than the page's current version.
    pub fn update_page(
        &self,
        id: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.put_json(&endpoints::page_path(id), body)
    }

    /// Fetches a content template by ID as raw JSON (v1 — no v2 equivalent).
    /// Used to resolve `page create --template-id`; the template's body is at
    /// `body.storage.value` in the response.
    pub fn get_template(&self, id: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::template_path(id))
    }

    /// Creates a content template from a pre-built request body (`name`,
    /// `templateType`, `body`, optional `description`/`space`) and returns
    /// the created template as raw JSON (v1 — no v2 equivalent).
    pub fn create_template(
        &self,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.post_json(endpoints::PATH_TEMPLATE, body)
    }

    /// Lists page-type content templates, optionally scoped to `space_key`,
    /// offset-paginated (v1 — no v2 equivalent).
    pub fn list_templates(
        &self,
        space_key: Option<&str>,
        limit: u32,
        start: u32,
    ) -> Result<serde_json::Value, ClientError> {
        let mut params = vec![("limit".to_string(), limit.to_string()), ("start".to_string(), start.to_string())];
        if let Some(space_key) = space_key {
            params.push(("spaceKey".to_string(), space_key.to_string()));
        }
        let query = serde_urlencoded::to_string(&params)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;
        self.get_json(&format!("{}?{query}", endpoints::PATH_TEMPLATE_PAGE))
    }

    /// Runs a CQL content search (v1 — no v2 equivalent yet) and returns the raw
    /// JSON response (`{"results": [...], "size": N, "_links": {...}}`).
    pub fn search_content(
        &self,
        cql: &str,
        limit: u32,
        start: u32,
    ) -> Result<serde_json::Value, ClientError> {
        let params = [
            ("cql", cql.to_string()),
            ("limit", limit.to_string()),
            ("start", start.to_string()),
        ];
        let query = serde_urlencoded::to_string(params)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;
        self.get_json(&format!("{}?{query}", endpoints::PATH_CONTENT_SEARCH))
    }

    /// Lists spaces, cursor-paginated, and returns the raw JSON response
    /// (`{"results": [...], "_links": {"next": "..."}}`).
    pub fn list_spaces(
        &self,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let mut params = vec![("limit".to_string(), limit.to_string())];
        if let Some(cursor) = cursor {
            params.push(("cursor".to_string(), cursor.to_string()));
        }
        let query = serde_urlencoded::to_string(&params)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;
        self.get_json(&format!("{}?{query}", endpoints::PATH_SPACES))
    }

    /// Updates a template from a pre-built request body (`templateId`, `name`,
    /// `templateType`, `body`, optional `description`/`space`) and returns
    /// the updated template as raw JSON (v1 — no v2 equivalent). Unlike
    /// `update_page`, the template ID goes in the request *body*
    /// (`templateId`), not the URL path — `PUT /wiki/rest/api/template` is
    /// not itself ID-scoped.
    pub fn update_template(
        &self,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.put_json(endpoints::PATH_TEMPLATE, body)
    }

    /// Deletes a page by ID. `purge = false` (default) moves it to the trash
    /// (recoverable); `purge = true` permanently removes it, but only works
    /// on a page that is already trashed — not yet live-verified what
    /// happens if `purge = true` is passed for a page that isn't already
    /// trashed. Confluence returns 204 No Content on success, hence `Ok(())`
    /// rather than a response value.
    pub fn delete_page(&self, id: &str, purge: bool) -> Result<(), ClientError> {
        let path = endpoints::page_path(id);
        if purge {
            self.delete(&format!("{path}?purge=true"))
        } else {
            self.delete(&path)
        }
    }

    /// Permanently deletes a template by ID (204 No Content on success, no
    /// trash/recovery step unlike `delete_page` — Confluence's own docs
    /// describe content template deletion as immediate and permanent).
    pub fn delete_template(&self, id: &str) -> Result<(), ClientError> {
        self.delete(&endpoints::template_path(id))
    }

    fn delete(&self, path: &str) -> Result<(), ClientError> {
        let url = format!("{}{path}", self.base_url);
        Self::status_error(self.send(|| self.http.delete(&url))?)?;
        Ok(())
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", self.base_url);
        Self::json_or_status_error(self.send(|| self.http.get(&url).header("Accept", "application/json"))?)
    }

    fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", self.base_url);
        Self::json_or_status_error(self.send(|| self.http.post(&url).header("Accept", "application/json").json(body))?)
    }

    fn put_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", self.base_url);
        Self::json_or_status_error(self.send(|| self.http.put(&url).header("Accept", "application/json").json(body))?)
    }

    /// Sends the request `build` makes with the current token. On a 401 renews
    /// the token once (if a renewer is set) and sends a rebuilt request: a 401
    /// means Confluence did not process it, so repeating a POST is safe.
    fn send(
        &self,
        build: impl Fn() -> reqwest::blocking::RequestBuilder,
    ) -> Result<reqwest::blocking::Response, ClientError> {
        self.token
            .send(
                |token| build().bearer_auth(token).send(),
                |response| matches!(response, Ok(r) if r.status() == reqwest::StatusCode::UNAUTHORIZED),
            )
            .map_err(|e| ClientError::Renewal(Box::new(e)))?
            .map_err(|e| ClientError::Request(e.to_string()))
    }

    fn status_error(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response, ClientError> {
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(ClientError::Status {
                status: status.as_u16(),
                body,
            });
        }
        Ok(response)
    }

    fn json_or_status_error(
        response: reqwest::blocking::Response,
    ) -> Result<serde_json::Value, ClientError> {
        Self::status_error(response)?
            .json::<serde_json::Value>()
            .map_err(|e| ClientError::Request(e.to_string()))
    }
}
