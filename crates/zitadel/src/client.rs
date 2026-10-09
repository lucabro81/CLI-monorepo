//! HTTP client for the ZITADEL APIs (blocking reqwest). Every method returns the
//! raw JSON response as `serde_json::Value`; shaping/projection happens in
//! `context::print_json` via `--select`.

use std::cell::RefCell;

use crate::auth::Credentials;
use crate::endpoints;
use crate::error::CliError;

/// Renews the token the API answered 401 to (the argument) and returns the new
/// one; errors are already mapped for the user (issue #240).
pub type Renewer = Box<dyn Fn(&str) -> Result<String, CliError>>;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("status {status}: {body}")]
    Status { status: u16, body: String },
    /// Renewing the token after a 401 failed; carries the error for the user.
    #[error("{0}")]
    Renewal(Box<CliError>),
}

pub struct ZitadelClient {
    instance_url: String,
    access_token: RefCell<String>,
    renewer: Option<Renewer>,
    http: reqwest::blocking::Client,
}

impl ZitadelClient {
    pub fn new(instance_url: &str, credentials: &Credentials) -> Self {
        Self {
            instance_url: instance_url.to_string(),
            access_token: RefCell::new(credentials.access_token.clone()),
            renewer: None,
            http: reqwest::blocking::Client::new(),
        }
    }

    /// On a 401 the client calls `renewer` once with the rejected token and
    /// repeats the request with the token it returns (issue #240: a token
    /// revoked before it expires). A 401 means the request was not processed,
    /// so repeating a POST is safe.
    #[must_use]
    pub fn with_renewer(mut self, renewer: Renewer) -> Self {
        self.renewer = Some(renewer);
        self
    }

    /// The authenticated identity (service user or human): `GET /auth/v1/users/me`.
    pub fn get_current_user(&self) -> Result<serde_json::Value, ClientError> {
        self.get_json(format!("{}{}", self.instance_url, endpoints::AUTH_USERS_ME_PATH))
    }

    /// The calling identity's administrator memberships and roles:
    /// `POST /auth/v1/memberships/me/_search`.
    pub fn list_my_memberships(&self) -> Result<serde_json::Value, ClientError> {
        self.post_json(endpoints::AUTH_MY_MEMBERSHIPS_SEARCH_PATH, &serde_json::json!({}))
    }

    /// Searches users (v2 ListUsers): `POST /v2/users` with a query/queries body.
    pub fn search_users(&self, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        self.post_json(endpoints::USERS_V2_PATH, body)
    }

    /// Searches organizations (v2 `ListOrganizations`): `POST /v2/organizations/_search`.
    pub fn list_organizations(&self, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        self.post_json(endpoints::ORGANIZATIONS_SEARCH_V2_PATH, body)
    }

    /// Lists projects (v2 `ListProjects`, Connect path — see `endpoints`).
    pub fn list_projects(&self, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        self.post_json(endpoints::PROJECTS_LIST_V2_PATH, body)
    }

    /// Lists authorizations (v2 `ListAuthorizations`, Connect path — see `endpoints`).
    pub fn list_authorizations(&self, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        self.post_json(endpoints::AUTHORIZATIONS_LIST_V2_PATH, body)
    }

    /// A user's identity provider links (v2 `ListIDPLinks`): `POST /v2/users/{userId}/links/_search`.
    pub fn list_idp_links(&self, user_id: &str, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        let mut url = self.url_with_segment(endpoints::USERS_V2_PATH, user_id)?;
        url.path_segments_mut()
            .map_err(|()| ClientError::Request("instance URL cannot have path segments".to_string()))?
            .extend(endpoints::IDP_LINKS_SEARCH_SEGMENTS);
        self.post_json_to(url, body)
    }

    /// An identity provider by id (v2 `GetIDPByID`): `GET /v2/idps/{idpId}`.
    pub fn get_idp(&self, idp_id: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(self.url_with_segment(endpoints::IDPS_V2_PATH, idp_id)?)
    }

    /// A single user by id (v2 `GetUserByID`): `GET /v2/users/{userId}`.
    pub fn get_user(&self, user_id: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(self.url_with_segment(endpoints::USERS_V2_PATH, user_id)?)
    }

    /// `instance_url + base_path + "/" + segment`, with `segment` percent-encoded
    /// as one path segment (so an id containing `/` can't reach another endpoint).
    fn url_with_segment(&self, base_path: &str, segment: &str) -> Result<reqwest::Url, ClientError> {
        let mut url = reqwest::Url::parse(&format!("{}{base_path}", self.instance_url))
            .map_err(|e| ClientError::Request(format!("invalid URL: {e}")))?;
        url.path_segments_mut()
            .map_err(|()| ClientError::Request("instance URL cannot have path segments".to_string()))?
            .push(segment);
        Ok(url)
    }

    /// Any request, for e2e fixtures (create/delete) that the CLI itself never sends.
    #[cfg(test)]
    pub(crate) fn send_for_tests(
        &self,
        method: &reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", self.instance_url);
        self.send(|| {
            let request = self.http.request(method.clone(), &url);
            match body {
                Some(body) => request.json(body),
                None => request,
            }
        })
    }

    fn get_json(&self, url: impl reqwest::IntoUrl) -> Result<serde_json::Value, ClientError> {
        let url = url.into_url().map_err(|e| ClientError::Request(e.to_string()))?;
        self.send(|| self.http.get(url.clone()))
    }

    fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        self.post_json_to(format!("{}{path}", self.instance_url), body)
    }

    fn post_json_to(&self, url: impl reqwest::IntoUrl, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        let url = url.into_url().map_err(|e| ClientError::Request(e.to_string()))?;
        self.send(|| self.http.post(url.clone()).json(body))
    }

    /// Sends the request `build` makes with the current token; on a 401 renews
    /// the token once (if a renewer is set) and sends a rebuilt request.
    fn send(&self, build: impl Fn() -> reqwest::blocking::RequestBuilder) -> Result<serde_json::Value, ClientError> {
        let response = self.send_once(&build)?;
        if response.status() != reqwest::StatusCode::UNAUTHORIZED {
            return Self::into_json(response);
        }
        let Some(renewer) = &self.renewer else {
            return Self::into_json(response);
        };
        let rejected = self.access_token.borrow().clone();
        let fresh_token = renewer(&rejected).map_err(|e| ClientError::Renewal(Box::new(e)))?;
        *self.access_token.borrow_mut() = fresh_token;
        Self::into_json(self.send_once(&build)?)
    }

    fn send_once(
        &self,
        build: &impl Fn() -> reqwest::blocking::RequestBuilder,
    ) -> Result<reqwest::blocking::Response, ClientError> {
        build()
            .bearer_auth(self.access_token.borrow().as_str())
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .map_err(|e| ClientError::Request(e.to_string()))
    }

    fn into_json(response: reqwest::blocking::Response) -> Result<serde_json::Value, ClientError> {
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(ClientError::Status {
                status: status.as_u16(),
                body,
            });
        }
        response
            .json()
            .map_err(|e| ClientError::Request(format!("invalid JSON response: {e}")))
    }
}

#[cfg(test)]
#[path = "tests/client_tests.rs"]
mod tests;
