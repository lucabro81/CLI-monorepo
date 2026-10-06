//! Shared Atlassian Cloud OAuth 2.0 infrastructure, used by every crate in
//! this workspace that authenticates against `auth.atlassian.com` /
//! `api.atlassian.com` (currently `jira` and `confluence` — see this crate's
//! own module docs and root `CLAUDE.md`'s "Shared library" sections).
//!
//! Deliberately **not** used by `bitbucket` (a different OAuth provider
//! entirely — its own native `client_credentials`-only consumer, no PKCE, no
//! `cloud_id`) or `atlassian-admin` (a static API key, no OAuth at all).

pub mod endpoints;
mod oauth;

pub use oauth::{
    app_config_path, authorization_url, check_identity, complete_remote_login,
    get_granted_scopes, load_credentials, login, login_client_credentials, now_unix,
    refresh, renew, save_credentials, start_remote_login, AppConfig, Credentials, Identity, LoginError, OAuthConfig,
    OAuthConfigError,
};
