//! Handler for the `listen` command: a long-running streaming-pull listener
//! on a Pub/Sub subscription.
//!
//! This is the only async corner of an otherwise fully synchronous crate
//! (`reqwest::blocking` everywhere else) — `google-cloud-pubsub` is
//! tokio-async only. The async runtime is built and torn down entirely
//! inside `run_listen`, so `main.rs` and every other command handler stay
//! synchronous.

use std::sync::Arc;
use std::time::Duration;

use google_cloud_auth::credentials::{CacheableResource, CredentialsProvider, EntityTag};
use google_cloud_auth::errors::CredentialsError;
use google_cloud_pubsub::client::Subscriber;
use google_cloud_pubsub::model::Message;
use http::{Extensions, HeaderMap, HeaderValue, header::AUTHORIZATION};
use tokio::sync::RwLock;

use crate::context::authenticated_credentials;
use crate::error::CliError;
use crate::events_client::EventsClient;

/// How often the background task re-checks whether the access token needs
/// renewing. `context::authenticated_credentials` only actually renews when
/// within 60s of expiry, so polling this often is cheap (a config file read
/// in the common case) and keeps `listen` running indefinitely without a
/// manual restart when the token would otherwise expire (~1h).
const REFRESH_POLL_INTERVAL: Duration = Duration::from_secs(300);

/// How often the background task renews the Workspace Events subscription's
/// TTL. Comfortably shorter than the ~4h TTL observed live, so a renewal is
/// never close to missing the deadline even if one attempt fails transiently.
const SUBSCRIPTION_RENEW_INTERVAL: Duration = Duration::from_secs(30 * 60);

pub fn run_listen(
    pubsub_subscription: String,
    workspace_events_subscription: String,
    max_messages: Option<u32>,
) -> Result<(), CliError> {
    let initial_token = authenticated_credentials()?.access_token;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| CliError::PubsubSubscribeFailed {
        reason: e.to_string(),
    })?;
    runtime.block_on(listen(
        pubsub_subscription,
        workspace_events_subscription,
        max_messages,
        initial_token,
    ))
}

async fn listen(
    pubsub_subscription: String,
    workspace_events_subscription: String,
    max_messages: Option<u32>,
    initial_token: String,
) -> Result<(), CliError> {
    let token = Arc::new(RwLock::new(initial_token));

    let subscriber = Subscriber::builder()
        .with_credentials(SharedTokenCredentials { token: token.clone() })
        .build()
        .await
        .map_err(|e| CliError::PubsubSubscribeFailed { reason: e.to_string() })?;

    eprintln!(
        "listening on {pubsub_subscription} (pid {}, send SIGTERM/SIGINT to stop)",
        std::process::id()
    );

    let mut stream = subscriber.subscribe(pubsub_subscription).build();
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|e| CliError::PubsubSubscribeFailed { reason: e.to_string() })?;
    let mut received: u32 = 0;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => return Ok(()),
            _ = sigterm.recv() => return Ok(()),
            () = tokio::time::sleep(REFRESH_POLL_INTERVAL) => {
                refresh_token(&token).await?;
            }
            () = tokio::time::sleep(SUBSCRIPTION_RENEW_INTERVAL) => {
                renew_subscription(&token, &workspace_events_subscription).await?;
            }
            next = stream.next() => {
                match next {
                    None => return Ok(()),
                    Some(Err(e)) => {
                        return Err(CliError::PubsubSubscribeFailed { reason: e.to_string() });
                    }
                    Some(Ok((message, handler))) => {
                        println!("{}", message_to_json(&message));
                        handler.ack();
                        received += 1;
                        if max_messages.is_some_and(|max| received >= max) {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

/// Re-loads (and, if needed, renews) credentials from disk and writes the
/// resulting access token into the shared state read by `SharedTokenCredentials`.
async fn refresh_token(token: &Arc<RwLock<String>>) -> Result<(), CliError> {
    let credentials = tokio::task::spawn_blocking(authenticated_credentials)
        .await
        .map_err(|e| CliError::PubsubSubscribeFailed { reason: e.to_string() })??;
    *token.write().await = credentials.access_token;
    Ok(())
}

/// Renews the Workspace Events subscription's TTL so it does not expire
/// (~4h after creation/last renewal) while `listen` keeps running.
async fn renew_subscription(
    token: &Arc<RwLock<String>>,
    workspace_events_subscription: &str,
) -> Result<(), CliError> {
    let access_token = token.read().await.clone();
    let subscription = workspace_events_subscription.to_string();
    tokio::task::spawn_blocking(move || {
        EventsClient::new(&access_token).renew_subscription(&subscription)
    })
    .await
    .map_err(|e| CliError::PubsubSubscribeFailed { reason: e.to_string() })?
    .map_err(crate::events_client::EventsClientError::into_workspace_events_error)?;
    Ok(())
}

fn message_to_json(message: &Message) -> serde_json::Value {
    let data = serde_json::from_slice::<serde_json::Value>(&message.data).unwrap_or_else(|_| {
        serde_json::Value::String(String::from_utf8_lossy(&message.data).into_owned())
    });
    serde_json::json!({
        "messageId": message.message_id,
        "publishTime": message.publish_time,
        "attributes": message.attributes,
        "data": data,
    })
}

/// Adapter exposing a shared, refreshable access token as `google-cloud-auth`
/// `Credentials`, so the Pub/Sub subscriber can reuse this CLI's existing
/// OAuth token instead of Application Default Credentials.
#[derive(Debug)]
struct SharedTokenCredentials {
    token: Arc<RwLock<String>>,
}

impl CredentialsProvider for SharedTokenCredentials {
    async fn headers(
        &self,
        _extensions: Extensions,
    ) -> Result<CacheableResource<HeaderMap>, CredentialsError> {
        let token = self.token.read().await.clone();
        let mut headers = HeaderMap::new();
        let value = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|e| CredentialsError::from_msg(false, e.to_string()))?;
        headers.insert(AUTHORIZATION, value);
        Ok(CacheableResource::New {
            entity_tag: EntityTag::new(),
            data: headers,
        })
    }

    async fn universe_domain(&self) -> Option<String> {
        Some("googleapis.com".to_string())
    }
}
