#![allow(clippy::unwrap_used, clippy::expect_used)]

use clap::Parser;

use super::{AuthCommand, Cli, Command, MessagesCommand, SpacesCommand, SubscriptionCommand};

#[test]
fn parses_auth_login_with_no_flags() {
    let cli = Cli::parse_from(["google-chat", "auth", "login"]);

    assert!(matches!(
        cli.command,
        Command::Auth {
            command: AuthCommand::Login { user: false }
        }
    ));
}

#[test]
fn parses_auth_login_with_user_flag() {
    let cli = Cli::parse_from(["google-chat", "auth", "login", "--user"]);

    assert!(matches!(
        cli.command,
        Command::Auth {
            command: AuthCommand::Login { user: true }
        }
    ));
}

#[test]
fn parses_init_with_no_flags() {
    let cli = Cli::parse_from(["google-chat", "init"]);

    assert!(matches!(
        cli.command,
        Command::Init {
            client_id: None,
            client_secret: None,
        }
    ));
}

#[test]
fn parses_init_with_client_id_and_secret() {
    let cli = Cli::parse_from([
        "google-chat",
        "init",
        "--client-id",
        "my-id",
        "--client-secret",
        "my-secret",
    ]);

    assert!(matches!(
        cli.command,
        Command::Init {
            client_id: Some(ref id),
            client_secret: Some(ref secret),
        } if id == "my-id" && secret == "my-secret"
    ));
}

#[test]
fn parses_doctor() {
    let cli = Cli::parse_from(["google-chat", "doctor"]);

    assert!(matches!(cli.command, Command::Doctor));
}

#[test]
fn doctor_with_select_flag() {
    let cli = Cli::parse_from([
        "google-chat",
        "doctor",
        "--select",
        "app_config.status,credentials.status",
    ]);

    assert!(matches!(cli.command, Command::Doctor));
    assert_eq!(
        cli.select,
        Some("app_config.status,credentials.status".to_string())
    );
}

#[test]
fn parses_spaces_list_with_no_optional_flags() {
    let cli = Cli::parse_from(["google-chat", "spaces", "list"]);

    assert!(matches!(
        cli.command,
        Command::Spaces {
            command: SpacesCommand::List {
                page_size: 100,
                page_token: None,
            }
        }
    ));
}

#[test]
fn parses_spaces_list_with_page_size_and_token() {
    let cli = Cli::parse_from([
        "google-chat",
        "spaces",
        "list",
        "--page-size",
        "20",
        "--page-token",
        "abc123",
    ]);

    assert!(matches!(
        cli.command,
        Command::Spaces {
            command: SpacesCommand::List {
                page_size: 20,
                page_token: Some(ref token),
            }
        } if token == "abc123"
    ));
}

#[test]
fn rejects_unknown_spaces_subcommand() {
    let result = Cli::try_parse_from(["google-chat", "spaces", "bogus"]);

    assert!(result.is_err());
}

#[test]
fn parses_messages_list_with_only_required_space_flag() {
    let cli = Cli::parse_from(["google-chat", "messages", "list", "--space", "AAQA-_d58OQ"]);

    assert!(matches!(
        cli.command,
        Command::Messages {
            command: MessagesCommand::List {
                ref space,
                page_size: 100,
                page_token: None,
                order_by: None,
            }
        } if space == "AAQA-_d58OQ"
    ));
}

#[test]
fn parses_messages_list_with_all_flags() {
    let cli = Cli::parse_from([
        "google-chat",
        "messages",
        "list",
        "--space",
        "spaces/AAQA-_d58OQ",
        "--page-size",
        "20",
        "--page-token",
        "abc123",
        "--order-by",
        "createTime DESC",
    ]);

    assert!(matches!(
        cli.command,
        Command::Messages {
            command: MessagesCommand::List {
                ref space,
                page_size: 20,
                page_token: Some(ref token),
                order_by: Some(ref order),
            }
        } if space == "spaces/AAQA-_d58OQ" && token == "abc123" && order == "createTime DESC"
    ));
}

#[test]
fn rejects_messages_list_without_space_flag() {
    let result = Cli::try_parse_from(["google-chat", "messages", "list"]);

    assert!(result.is_err());
}

#[test]
fn rejects_unknown_messages_subcommand() {
    let result = Cli::try_parse_from(["google-chat", "messages", "bogus"]);

    assert!(result.is_err());
}

#[test]
fn parses_messages_send_with_space_and_text() {
    let cli = Cli::parse_from([
        "google-chat",
        "messages",
        "send",
        "--space",
        "spaces/AAQA-_d58OQ",
        "--text",
        "hello from the agent",
    ]);

    assert!(matches!(
        cli.command,
        Command::Messages {
            command: MessagesCommand::Send {
                ref space,
                ref text,
            }
        } if space == "spaces/AAQA-_d58OQ" && text == "hello from the agent"
    ));
}

#[test]
fn rejects_messages_send_without_text_flag() {
    let result = Cli::try_parse_from([
        "google-chat",
        "messages",
        "send",
        "--space",
        "spaces/AAQA-_d58OQ",
    ]);

    assert!(result.is_err());
}

#[test]
fn rejects_messages_send_without_space_flag() {
    let result = Cli::try_parse_from(["google-chat", "messages", "send", "--text", "hi"]);

    assert!(result.is_err());
}

#[test]
fn parses_global_select_flag_before_subcommand() {
    let cli = Cli::parse_from(["google-chat", "--select", "foo,bar", "auth", "login"]);

    assert_eq!(cli.select, Some("foo,bar".to_string()));
}

#[test]
fn parses_global_select_flag_after_subcommand() {
    let cli = Cli::parse_from(["google-chat", "auth", "login", "--select", "foo,bar"]);

    assert_eq!(cli.select, Some("foo,bar".to_string()));
}

#[test]
fn rejects_unknown_top_level_command() {
    let result = Cli::try_parse_from(["google-chat", "bogus"]);

    assert!(result.is_err());
}

#[test]
fn rejects_unknown_auth_subcommand() {
    let result = Cli::try_parse_from(["google-chat", "auth", "bogus"]);

    assert!(result.is_err());
}

#[test]
fn parses_subscription_create_with_required_flags_only() {
    let cli = Cli::parse_from([
        "google-chat",
        "subscription",
        "create",
        "--space",
        "spaces/AAQA-_d58OQ",
        "--topic",
        "projects/p/topics/t",
        "--pubsub-subscription",
        "projects/p/subscriptions/s",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::Create {
                ref space,
                ref topic,
                ref pubsub_subscription,
                ref event_type,
            }
        } if space == "spaces/AAQA-_d58OQ"
            && topic == "projects/p/topics/t"
            && pubsub_subscription == "projects/p/subscriptions/s"
            && event_type == &["google.workspace.chat.message.v1.created".to_string()]
    ));
}

#[test]
fn parses_subscription_create_with_repeated_event_type() {
    let cli = Cli::parse_from([
        "google-chat",
        "subscription",
        "create",
        "--space",
        "spaces/AAQA-_d58OQ",
        "--topic",
        "projects/p/topics/t",
        "--pubsub-subscription",
        "projects/p/subscriptions/s",
        "--event-type",
        "google.workspace.chat.message.v1.created",
        "--event-type",
        "google.workspace.chat.message.v1.updated",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::Create {
                ref event_type,
                ..
            }
        } if event_type
            == &[
                "google.workspace.chat.message.v1.created".to_string(),
                "google.workspace.chat.message.v1.updated".to_string(),
            ]
    ));
}

#[test]
fn rejects_subscription_create_without_required_flags() {
    let result = Cli::try_parse_from([
        "google-chat",
        "subscription",
        "create",
        "--space",
        "spaces/AAQA-_d58OQ",
    ]);

    assert!(result.is_err());
}

#[test]
fn rejects_unknown_subscription_subcommand() {
    let result = Cli::try_parse_from(["google-chat", "subscription", "bogus"]);

    assert!(result.is_err());
}

#[test]
fn parses_subscription_delete_with_name() {
    let cli = Cli::parse_from([
        "google-chat",
        "subscription",
        "delete",
        "--name",
        "subscriptions/chat-spaces-abc",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::Delete { ref name }
        } if name == "subscriptions/chat-spaces-abc"
    ));
}

#[test]
fn rejects_subscription_delete_without_name_flag() {
    let result = Cli::try_parse_from(["google-chat", "subscription", "delete"]);

    assert!(result.is_err());
}

#[test]
fn parses_listen_with_only_required_flags() {
    let cli = Cli::parse_from([
        "google-chat",
        "listen",
        "--pubsub-subscription",
        "projects/p/subscriptions/s",
        "--workspace-events-subscription",
        "subscriptions/chat-spaces-abc",
    ]);

    assert!(matches!(
        cli.command,
        Command::Listen {
            ref pubsub_subscription,
            ref workspace_events_subscription,
            max_messages: None,
        } if pubsub_subscription == "projects/p/subscriptions/s"
            && workspace_events_subscription == "subscriptions/chat-spaces-abc"
    ));
}

#[test]
fn parses_listen_with_max_messages() {
    let cli = Cli::parse_from([
        "google-chat",
        "listen",
        "--pubsub-subscription",
        "projects/p/subscriptions/s",
        "--workspace-events-subscription",
        "subscriptions/chat-spaces-abc",
        "--max-messages",
        "3",
    ]);

    assert!(matches!(
        cli.command,
        Command::Listen {
            ref pubsub_subscription,
            ref workspace_events_subscription,
            max_messages: Some(3),
        } if pubsub_subscription == "projects/p/subscriptions/s"
            && workspace_events_subscription == "subscriptions/chat-spaces-abc"
    ));
}

#[test]
fn rejects_listen_without_workspace_events_subscription_flag() {
    let result = Cli::try_parse_from([
        "google-chat",
        "listen",
        "--pubsub-subscription",
        "projects/p/subscriptions/s",
    ]);

    assert!(result.is_err());
}

#[test]
fn rejects_listen_without_pubsub_subscription_flag() {
    let result = Cli::try_parse_from(["google-chat", "listen"]);

    assert!(result.is_err());
}
