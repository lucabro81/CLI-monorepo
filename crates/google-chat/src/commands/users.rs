//! Handler for the `users` command group (`users get`).
//!
//! Delegates to `people_client::PeopleClient`, reusing the same OAuth access
//! token as `GoogleChatClient`/`EventsClient` (different scope, different
//! API entirely — see `people_client.rs`'s module doc for why the Chat API
//! itself can't serve this).

use crate::cli::UsersCommand;
use crate::context::{authenticated_credentials, print_json};
use crate::error::CliError;
use crate::people_client::PeopleClient;

pub fn run(command: UsersCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        UsersCommand::Get { user } => {
            let credentials = authenticated_credentials()?;
            let client = PeopleClient::new(&credentials.access_token);
            let value = client
                .get_user(&user)
                .map_err(crate::people_client::PeopleClientError::into_cli_error)?;
            // Exempt: a single People API profile object, fixed shape.
            print_json(&value, select.or_all())
        }
    }
}
