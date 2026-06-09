use crate::auth::Credentials;

#[derive(Debug)]
pub enum ClientError {
    Request(String),
    Status { status: u16, body: String },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Request(msg) => write!(f, "request failed: {msg}"),
            ClientError::Status { status, body } => {
                write!(f, "Jira returned status {status}: {body}")
            }
        }
    }
}

pub struct JiraClient {
    base_url: String,
    access_token: String,
    http: reqwest::blocking::Client,
}

impl JiraClient {
    pub fn new(credentials: &Credentials) -> Self {
        Self {
            base_url: format!("https://api.atlassian.com/ex/jira/{}", credentials.cloud_id),
            access_token: credentials.access_token.clone(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Fetches a Jira issue by key and returns the raw JSON response.
    pub fn get_issue(&self, key: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(&format!("/rest/api/3/issue/{key}"))
    }

    /// Returns the currently authenticated user as raw JSON.
    pub fn get_myself(&self) -> Result<serde_json::Value, ClientError> {
        self.get_json("/rest/api/3/myself")
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", self.base_url);

        let response = self
            .http
            .get(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| ClientError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(ClientError::Status {
                status: status.as_u16(),
                body,
            });
        }

        response
            .json::<serde_json::Value>()
            .map_err(|e| ClientError::Request(e.to_string()))
    }
}
