//! HTTP client for the ZITADEL APIs (blocking reqwest). Every method returns the
//! raw JSON response as `serde_json::Value`; shaping/projection happens in
//! `context::print_json` via `--select`.

use crate::auth::Credentials;
use crate::endpoints;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("status {status}: {body}")]
    Status { status: u16, body: String },
}

pub struct ZitadelClient {
    instance_url: String,
    access_token: String,
    http: reqwest::blocking::Client,
}

impl ZitadelClient {
    pub fn new(instance_url: &str, credentials: &Credentials) -> Self {
        Self {
            instance_url: instance_url.to_string(),
            access_token: credentials.access_token.clone(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// The authenticated identity (service user or human): `GET /auth/v1/users/me`.
    pub fn get_current_user(&self) -> Result<serde_json::Value, ClientError> {
        self.get_json(endpoints::AUTH_USERS_ME_PATH)
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

    fn get_json(&self, path: &str) -> Result<serde_json::Value, ClientError> {
        let response = self
            .http
            .get(format!("{}{path}", self.instance_url))
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .map_err(|e| ClientError::Request(e.to_string()))?;
        Self::into_json(response)
    }

    fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        let response = self
            .http
            .post(format!("{}{path}", self.instance_url))
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .json(body)
            .send()
            .map_err(|e| ClientError::Request(e.to_string()))?;
        Self::into_json(response)
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
