use std::env::VarError;

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub base_url: String,
    pub email: String,
    pub api_token: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    MissingVar(&'static str),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingVar(name) => {
                write!(f, "missing environment variable {name}")
            }
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_getter(|key| std::env::var(key))
    }

    fn from_getter(get: impl Fn(&str) -> Result<String, VarError>) -> Result<Self, ConfigError> {
        Ok(Config {
            base_url: get("JIRA_BASE_URL").map_err(|_| ConfigError::MissingVar("JIRA_BASE_URL"))?,
            email: get("JIRA_EMAIL").map_err(|_| ConfigError::MissingVar("JIRA_EMAIL"))?,
            api_token: get("JIRA_API_TOKEN")
                .map_err(|_| ConfigError::MissingVar("JIRA_API_TOKEN"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn getter(vars: HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Result<String, VarError> {
        move |key: &str| {
            vars.get(key)
                .map(|v| v.to_string())
                .ok_or(VarError::NotPresent)
        }
    }

    #[test]
    fn builds_config_when_all_vars_present() {
        let vars = HashMap::from([
            ("JIRA_BASE_URL", "https://example.atlassian.net"),
            ("JIRA_EMAIL", "user@example.com"),
            ("JIRA_API_TOKEN", "secret-token"),
        ]);

        let config = Config::from_getter(getter(vars)).expect("config should build");

        assert_eq!(
            config,
            Config {
                base_url: "https://example.atlassian.net".to_string(),
                email: "user@example.com".to_string(),
                api_token: "secret-token".to_string(),
            }
        );
    }

    #[test]
    fn errors_when_base_url_missing() {
        let vars = HashMap::from([
            ("JIRA_EMAIL", "user@example.com"),
            ("JIRA_API_TOKEN", "secret-token"),
        ]);

        let err = Config::from_getter(getter(vars)).unwrap_err();

        assert_eq!(err, ConfigError::MissingVar("JIRA_BASE_URL"));
    }

    #[test]
    fn errors_when_email_missing() {
        let vars = HashMap::from([
            ("JIRA_BASE_URL", "https://example.atlassian.net"),
            ("JIRA_API_TOKEN", "secret-token"),
        ]);

        let err = Config::from_getter(getter(vars)).unwrap_err();

        assert_eq!(err, ConfigError::MissingVar("JIRA_EMAIL"));
    }

    #[test]
    fn errors_when_api_token_missing() {
        let vars = HashMap::from([
            ("JIRA_BASE_URL", "https://example.atlassian.net"),
            ("JIRA_EMAIL", "user@example.com"),
        ]);

        let err = Config::from_getter(getter(vars)).unwrap_err();

        assert_eq!(err, ConfigError::MissingVar("JIRA_API_TOKEN"));
    }
}
