use clap::{Parser, Subcommand};

/// Jira CLI for LLM agents — query Jira issues from the command line.
#[derive(Debug, Parser)]
#[command(name = "jira", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Work with Jira issues
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    /// Fetch a single issue by key (e.g. PROJ-123) and print it as JSON
    Get {
        /// Issue key, e.g. PROJ-123
        key: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_issue_get_with_key() {
        let cli = Cli::try_parse_from(["jira", "issue", "get", "PROJ-123"]).expect("should parse");

        match cli.command {
            Command::Issue {
                command: IssueCommand::Get { key },
            } => assert_eq!(key, "PROJ-123"),
        }
    }

    #[test]
    fn rejects_issue_get_without_key() {
        let result = Cli::try_parse_from(["jira", "issue", "get"]);

        assert!(result.is_err());
    }

    #[test]
    fn rejects_unknown_command() {
        let result = Cli::try_parse_from(["jira", "bogus"]);

        assert!(result.is_err());
    }
}
