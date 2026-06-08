mod cli;
mod client;
mod config;

use clap::Parser;
use cli::{Cli, Command, IssueCommand};
use client::JiraClient;
use config::Config;

fn main() {
    let cli = Cli::parse();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!("hint: set JIRA_BASE_URL, JIRA_EMAIL and JIRA_API_TOKEN environment variables");
            std::process::exit(1);
        }
    };

    let client = JiraClient::new(config);

    let result = match cli.command {
        Command::Issue { command } => match command {
            IssueCommand::Get { key } => client.get_issue(&key),
        },
    };

    match result {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}
