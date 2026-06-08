mod auth;
mod cli;
mod client;

use auth::OAuthConfig;
use clap::Parser;
use cli::{AuthCommand, Cli, Command, IssueCommand};
use client::JiraClient;

/// XDG-style config directory (`$XDG_CONFIG_HOME` or `~/.config`), used on every platform
/// so dev machines and headless deployment targets share the same layout.
fn config_dir() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return std::path::PathBuf::from(xdg);
    }
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".config")
}

fn credentials_path() -> std::path::PathBuf {
    auth::credentials_path(&config_dir())
}

fn oauth_config_or_exit() -> OAuthConfig {
    let path = auth::app_config_path(&config_dir());
    match OAuthConfig::load(&path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!(
                "hint: create {} with your Atlassian OAuth 2.0 (3LO) app credentials, e.g.:\n  {{\"client_id\": \"...\", \"client_secret\": \"...\"}}",
                path.display()
            );
            std::process::exit(1);
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let path = credentials_path();

    match cli.command {
        Command::Auth {
            command: AuthCommand::Login,
        } => {
            let oauth_config = oauth_config_or_exit();
            match auth::login(&oauth_config) {
                Ok(credentials) => {
                    if let Err(err) = auth::save_credentials(&path, &credentials) {
                        eprintln!("error: failed to save credentials: {err}");
                        std::process::exit(1);
                    }
                    println!("Logged in. Credentials saved to {}", path.display());
                }
                Err(err) => {
                    eprintln!("error: login failed: {err}");
                    std::process::exit(1);
                }
            }
        }
        Command::Issue { command } => {
            let oauth_config = oauth_config_or_exit();
            let credentials = match auth::load_credentials(&oauth_config, &path) {
                Ok(credentials) => credentials,
                Err(err) => {
                    eprintln!("error: not authenticated ({err})");
                    eprintln!("hint: run `jira auth login` first");
                    std::process::exit(1);
                }
            };

            let client = JiraClient::new(&credentials);

            let result = match command {
                IssueCommand::Get { key } => client.get_issue(&key),
            };

            match result {
                Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
                Err(err) => {
                    eprintln!("error: {err}");
                    std::process::exit(1);
                }
            }
        }
    }
}
