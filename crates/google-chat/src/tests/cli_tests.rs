#![allow(clippy::unwrap_used, clippy::expect_used)]

use clap::Parser;

use super::{
    AuthCommand, Cli, Command, MessagesCommand, SpaceMembersCommand, SpacesCommand,
    SubscriptionCommand, UsersCommand,
};

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
fn parses_spaces_members_list_with_no_optional_flags() {
    let cli = Cli::parse_from(["google-chat", "spaces", "members", "list", "--space", "AAQAtCLmaho"]);

    assert!(matches!(
        cli.command,
        Command::Spaces {
            command: SpacesCommand::Members {
                command: SpaceMembersCommand::List {
                    ref space,
                    page_size: 100,
                    page_token: None,
                }
            }
        } if space == "AAQAtCLmaho"
    ));
}

#[test]
fn parses_spaces_members_list_with_page_size_and_token() {
    let cli = Cli::parse_from([
        "google-chat",
        "spaces",
        "members",
        "list",
        "--space",
        "spaces/AAQAtCLmaho",
        "--page-size",
        "20",
        "--page-token",
        "abc123",
    ]);

    assert!(matches!(
        cli.command,
        Command::Spaces {
            command: SpacesCommand::Members {
                command: SpaceMembersCommand::List {
                    ref space,
                    page_size: 20,
                    page_token: Some(ref token),
                }
            }
        } if space == "spaces/AAQAtCLmaho" && token == "abc123"
    ));
}

#[test]
fn rejects_spaces_members_list_missing_space() {
    let result = Cli::try_parse_from(["google-chat", "spaces", "members", "list"]);
    assert!(result.is_err());
}

#[test]
fn parses_spaces_create_with_single_user() {
    let cli = Cli::parse_from(["google-chat", "spaces", "create", "--user", "colleague@example.com"]);

    assert!(matches!(
        cli.command,
        Command::Spaces {
            command: SpacesCommand::Create { ref user }
        } if user == &["colleague@example.com".to_string()]
    ));
}

#[test]
fn parses_spaces_create_with_repeated_user() {
    let cli = Cli::parse_from([
        "google-chat",
        "spaces",
        "create",
        "--user",
        "colleague@example.com",
        "--user",
        "other@example.com",
    ]);

    assert!(matches!(
        cli.command,
        Command::Spaces {
            command: SpacesCommand::Create { ref user }
        } if user == &[
            "colleague@example.com".to_string(),
            "other@example.com".to_string(),
        ]
    ));
}

#[test]
fn rejects_spaces_create_without_user() {
    let result = Cli::try_parse_from(["google-chat", "spaces", "create"]);
    assert!(result.is_err());
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
fn parses_messages_delete_with_name_only() {
    let cli = Cli::parse_from([
        "google-chat",
        "messages",
        "delete",
        "--name",
        "spaces/AAQA-_d58OQ/messages/abc123",
    ]);

    assert!(matches!(
        cli.command,
        Command::Messages {
            command: MessagesCommand::Delete {
                ref name,
                confirm,
                delete_threaded_replies,
            }
        } if name == "spaces/AAQA-_d58OQ/messages/abc123" && !confirm && !delete_threaded_replies
    ));
}

#[test]
fn parses_messages_delete_with_all_flags() {
    let cli = Cli::parse_from([
        "google-chat",
        "messages",
        "delete",
        "--name",
        "spaces/AAQA-_d58OQ/messages/abc123",
        "--confirm",
        "--delete-threaded-replies",
    ]);

    assert!(matches!(
        cli.command,
        Command::Messages {
            command: MessagesCommand::Delete {
                ref name,
                confirm,
                delete_threaded_replies,
            }
        } if name == "spaces/AAQA-_d58OQ/messages/abc123" && confirm && delete_threaded_replies
    ));
}

#[test]
fn rejects_messages_delete_without_name_flag() {
    let result = Cli::try_parse_from(["google-chat", "messages", "delete", "--confirm"]);

    assert!(result.is_err());
}

#[test]
fn parses_messages_update_with_name_and_text() {
    let cli = Cli::parse_from([
        "google-chat",
        "messages",
        "update",
        "--name",
        "spaces/AAQA-_d58OQ/messages/abc123",
        "--text",
        "corrected text",
    ]);

    assert!(matches!(
        cli.command,
        Command::Messages {
            command: MessagesCommand::Update {
                ref name,
                ref text,
            }
        } if name == "spaces/AAQA-_d58OQ/messages/abc123" && text == "corrected text"
    ));
}

#[test]
fn rejects_messages_update_without_name_flag() {
    let result = Cli::try_parse_from(["google-chat", "messages", "update", "--text", "hi"]);

    assert!(result.is_err());
}

#[test]
fn rejects_messages_update_without_text_flag() {
    let result = Cli::try_parse_from([
        "google-chat",
        "messages",
        "update",
        "--name",
        "spaces/AAQA-_d58OQ/messages/abc123",
    ]);

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
fn select_flag_is_none_and_select_all_false_when_absent() {
    let cli = Cli::parse_from(["google-chat", "auth", "login"]);

    assert!(cli.select.is_none());
    assert!(!cli.select_all);
}

#[test]
fn select_all_flag_parses() {
    let cli = Cli::parse_from(["google-chat", "--select-all", "auth", "login"]);

    assert!(cli.select_all);
    assert!(cli.select.is_none());
}

#[test]
fn select_and_select_all_together_are_rejected() {
    let result = Cli::try_parse_from([
        "google-chat",
        "--select",
        "foo",
        "--select-all",
        "auth",
        "login",
    ]);

    assert!(result.is_err(), "--select and --select-all should conflict");
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
                ref message_filter,
                allow_unfiltered,
            }
        } if space == "spaces/AAQA-_d58OQ"
            && topic == "projects/p/topics/t"
            && pubsub_subscription == "projects/p/subscriptions/s"
            && event_type == &["google.workspace.chat.message.v1.created".to_string()]
            && message_filter.is_none()
            && !allow_unfiltered
    ));
}

#[test]
fn parses_subscription_create_with_allow_unfiltered() {
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
        "--allow-unfiltered",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::Create {
                ref message_filter,
                allow_unfiltered,
                ..
            }
        } if message_filter.is_none() && allow_unfiltered
    ));
}

#[test]
fn rejects_subscription_create_with_message_filter_and_allow_unfiltered_together() {
    let result = Cli::try_parse_from([
        "google-chat",
        "subscription",
        "create",
        "--space",
        "spaces/AAQA-_d58OQ",
        "--topic",
        "projects/p/topics/t",
        "--pubsub-subscription",
        "projects/p/subscriptions/s",
        "--message-filter",
        "hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")",
        "--allow-unfiltered",
    ]);

    assert!(result.is_err());
}

#[test]
fn parses_subscription_create_with_message_filter() {
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
        "--message-filter",
        "hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::Create {
                ref message_filter,
                ..
            }
        } if message_filter.as_deref()
            == Some("hasPrefix(attributes.ce-subject, \"//chat.googleapis.com/spaces/AAQA-_d58OQ\")")
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
fn parses_subscription_get_with_name() {
    let cli = Cli::parse_from([
        "google-chat",
        "subscription",
        "get",
        "--name",
        "subscriptions/chat-spaces-abc",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::Get { ref name }
        } if name == "subscriptions/chat-spaces-abc"
    ));
}

#[test]
fn rejects_subscription_get_without_name_flag() {
    let result = Cli::try_parse_from(["google-chat", "subscription", "get"]);

    assert!(result.is_err());
}

#[test]
fn parses_subscription_list_with_required_event_type_only() {
    let cli = Cli::parse_from([
        "google-chat",
        "subscription",
        "list",
        "--event-type",
        "google.workspace.chat.message.v1.created",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::List {
                ref event_type,
                ref space,
                page_size,
                ref page_token,
            }
        } if event_type == &["google.workspace.chat.message.v1.created".to_string()]
            && space.is_none()
            && page_size == 50
            && page_token.is_none()
    ));
}

#[test]
fn parses_subscription_list_with_repeated_event_type() {
    let cli = Cli::parse_from([
        "google-chat",
        "subscription",
        "list",
        "--event-type",
        "google.workspace.chat.message.v1.created",
        "--event-type",
        "google.workspace.chat.message.v1.updated",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::List { ref event_type, .. }
        } if event_type
            == &[
                "google.workspace.chat.message.v1.created".to_string(),
                "google.workspace.chat.message.v1.updated".to_string(),
            ]
    ));
}

#[test]
fn parses_subscription_list_with_all_flags() {
    let cli = Cli::parse_from([
        "google-chat",
        "subscription",
        "list",
        "--event-type",
        "google.workspace.chat.message.v1.created",
        "--space",
        "spaces/AAQA-_d58OQ",
        "--page-size",
        "10",
        "--page-token",
        "tok123",
    ]);

    assert!(matches!(
        cli.command,
        Command::Subscription {
            command: SubscriptionCommand::List {
                ref space,
                page_size,
                ref page_token,
                ..
            }
        } if space.as_deref() == Some("spaces/AAQA-_d58OQ")
            && page_size == 10
            && page_token.as_deref() == Some("tok123")
    ));
}

#[test]
fn rejects_subscription_list_without_event_type() {
    let result = Cli::try_parse_from(["google-chat", "subscription", "list"]);

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

#[test]
fn parses_users_get_with_bare_id() {
    let cli = Cli::parse_from(["google-chat", "users", "get", "--user", "108506379394699518479"]);

    assert!(matches!(
        cli.command,
        Command::Users {
            command: UsersCommand::Get { ref user }
        } if user == "108506379394699518479"
    ));
}

#[test]
fn parses_users_get_with_full_resource_name() {
    let cli = Cli::parse_from([
        "google-chat",
        "users",
        "get",
        "--user",
        "users/108506379394699518479",
    ]);

    assert!(matches!(
        cli.command,
        Command::Users {
            command: UsersCommand::Get { ref user }
        } if user == "users/108506379394699518479"
    ));
}

#[test]
fn rejects_users_get_without_user_flag() {
    let result = Cli::try_parse_from(["google-chat", "users", "get"]);

    assert!(result.is_err());
}

#[test]
fn rejects_unknown_users_subcommand() {
    let result = Cli::try_parse_from(["google-chat", "users", "bogus"]);

    assert!(result.is_err());
}
