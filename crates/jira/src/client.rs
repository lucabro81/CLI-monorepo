use crate::config::Config;

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
    config: Config,
    http: reqwest::blocking::Client,
}

impl JiraClient {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            http: reqwest::blocking::Client::new(),
        }
    }

    pub fn get_issue(&self, key: &str) -> Result<serde_json::Value, ClientError> {
        let url = format!(
            "{}/rest/api/3/issue/{key}",
            self.config.base_url.trim_end_matches('/')
        );

        let response = self
            .http
            .get(&url)
            .basic_auth(&self.config.email, Some(&self.config.api_token))
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
