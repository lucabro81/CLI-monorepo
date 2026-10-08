//! Handler for the `init` command: writes `app.json`, logs in as the service
//! user or, with `--user <id>`, as that person through the browser, and runs
//! `doctor` for that identity as the final verification.
//!
//! Like `jira init` / `bitbucket init` (issue #228): it prints numbered console
//! setup steps and asks for whatever the flags didn't give
//! ([`resolve_init_inputs`]): the instance URL (only when app.json has none —
//! changing it discards every login), then the service user's whole key JSON
//! (hidden on a terminal) or, for people, the Native app client id. A flag skips
//! its question; an empty answer writes nothing; on piped stdin every answer is
//! a plain line.
//!
//! `init --user-app` (issue #195) writes `app.json` the same way but logs nobody
//! in, and prints only the `app_config` check (`user_app_report`): the Native app
//! can be set up where nobody can open a browser, people log in later with
//! `auth login --user <id> --remote`.
//!
//! Re-running merges with the existing `app.json`: values not asked or given
//! keep their current value. Narrative progress and questions go to stderr;
//! stdout carries only the JSON report.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use serde_json::{Value, json};

use crate::auth::{self, AppConfig, AppConfigError, Identity, ServiceUserKey};
use crate::commands::doctor;
use crate::context::{config_dir, print_json};
use crate::error::CliError;

pub fn run_init(
    identity: &Identity,
    instance_url: Option<&str>,
    key_file: Option<&Path>,
    client_id: Option<&str>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    let mode = match identity {
        Identity::Service => InitMode::Service,
        Identity::User(_) => InitMode::User,
    };
    eprintln!("{}", mode.steps());
    let config_dir = config_dir()?;
    let existing = load_existing(&config_dir);
    let inputs = resolve_init_inputs(mode, instance_url, key_file, client_id, existing.as_ref(), ask_on_stdin)?;
    let config = write_config(&config_dir, existing, inputs)?;

    let creds_path = auth::credentials_path(&config_dir, identity);
    let credentials = match identity {
        Identity::Service => Some(
            auth::login_service_user(&config).map_err(|e| CliError::LoginFailed { reason: e.to_string() })?,
        ),
        Identity::User(id) => {
            eprintln!("Starting the browser login as {id}.");
            Some(
                auth::login_user(&config)
                    .map_err(|e| CliError::UserLoginFailed { reason: e.to_string(), id: id.to_string() })?,
            )
        }
    };
    if let Some(credentials) = credentials {
        auth::save_credentials(&creds_path, &credentials).map_err(|e| {
            CliError::SaveCredentialsFailed {
                path: creds_path.display().to_string(),
                reason: e.to_string(),
            }
        })?;
        eprintln!("Logged in. Credentials saved to {}", creds_path.display());
    }

    let (report, all_ok) = doctor::run_doctor_in(&config_dir, identity);
    // Same exemption as `doctor`: full report unless an explicit --select is given.
    print_json(&report, select.or_all())?;
    if !all_ok {
        return Err(CliError::DoctorCheckFailed);
    }
    Ok(())
}

/// The existing `app.json`, if any; an invalid one is reported and ignored.
fn load_existing(config_dir: &Path) -> Option<AppConfig> {
    let app_path = auth::app_config_path(config_dir);
    match AppConfig::load(&app_path) {
        Ok(config) => Some(config),
        Err(AppConfigError::NotFound(_)) => None,
        Err(e) => {
            eprintln!("Ignoring the existing invalid {} ({e}).", app_path.display());
            None
        }
    }
}

/// Builds `app.json` from `inputs` merged with the existing file, writes it, and
/// discards every stored login when the instance URL changed.
fn write_config(config_dir: &Path, existing: Option<AppConfig>, inputs: InitInputs) -> Result<AppConfig, CliError> {
    let app_path = auth::app_config_path(config_dir);
    let previous = existing.clone();
    let config = build_app_config(
        existing,
        inputs.instance_url.as_deref(),
        inputs.service_user,
        inputs.client_id.as_deref(),
    )?;
    write_app_config(&app_path, &config)?;
    eprintln!("Wrote {}", app_path.display());

    if instance_changed(previous.as_ref(), &config) {
        discard_instance_credentials(config_dir)?;
        eprintln!("Instance URL changed: removed the credentials of the previous instance.");
    }
    Ok(config)
}

const SERVICE_STEPS: &str = "\
=== zitadel init: service user setup (the default identity) ===

Step 1: In the ZITADEL console: Users -> Service Users -> New (e.g. \"zitadel-cli\", access token type JWT).
Step 2: Grant it only what your commands need. To read users and their project roles, add it
        under Organization -> Managers with a read-only role such as ORG_OWNER_VIEWER; add
        administrator roles only if the CLI should change things.
Step 3: On the service user: Keys -> New, type JSON, and download the file.
Step 4: Paste the whole content of that file when asked (it is one line; the input is hidden).

For people (commands run with --user <USER_ID>), run `zitadel init --user-app` instead.
";

const NATIVE_APP_STEPS: &str = "\
=== zitadel init --user-app / --user: Native app setup (every person logs in with it) ===

Step 1: In a ZITADEL project: Applications -> New -> type Native.
Step 2: Authentication method: PKCE.
Step 3: Redirect URIs: http://localhost:8080/callback (turn on development mode: it is http) and,
        for people logging in from elsewhere, the HTTPS URL that receives the code, e.g.
        https://<your server>/login/callback (passed to auth login --remote --redirect-uri).
Step 4: Token settings: enable Refresh Token.
Step 5: Copy the application's Client ID.
";

/// Which identity `init` sets up, deciding what it asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InitMode {
    /// `zitadel init`: the service user (instance URL, key JSON).
    Service,
    /// `zitadel init --user <id>`: the Native app, then that person's browser login.
    User,
    /// `zitadel init --user-app`: the Native app, nobody logged in.
    UserApp,
}

impl InitMode {
    fn steps(self) -> &'static str {
        match self {
            InitMode::Service => SERVICE_STEPS,
            InitMode::User | InitMode::UserApp => NATIVE_APP_STEPS,
        }
    }
}

/// One value `init` may ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Question {
    InstanceUrl,
    ServiceUserKey,
    ClientId,
}

impl Question {
    fn label(self) -> &'static str {
        match self {
            Question::InstanceUrl => "ZITADEL instance URL (e.g. https://acme.zitadel.cloud)",
            Question::ServiceUserKey => "Service user key JSON (paste the whole downloaded file)",
            Question::ClientId => "Native app client ID",
        }
    }

    /// The field and flag an empty answer names.
    fn field_and_flag(self) -> (&'static str, &'static str) {
        match self {
            Question::InstanceUrl => ("Instance URL", "--instance-url"),
            Question::ServiceUserKey => ("Service user key", "--key-file <path>"),
            Question::ClientId => ("Native app client ID", "--client-id"),
        }
    }
}

/// What `init` writes on top of the existing app.json (`None` keeps the current value).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InitInputs {
    pub instance_url: Option<String>,
    pub service_user: Option<ServiceUserKey>,
    pub client_id: Option<String>,
}

/// Decides what `mode` needs that neither the flags nor (for the instance URL)
/// the existing app.json give, and asks for it through `ask`, in order: instance
/// URL, then the service user key (service) or the Native app client id (people).
/// Answers are trimmed; an empty one is `CliError::EmptyInput`; a pasted key is
/// validated like `--key-file`.
pub(crate) fn resolve_init_inputs(
    mode: InitMode,
    instance_url: Option<&str>,
    key_file: Option<&Path>,
    client_id: Option<&str>,
    existing: Option<&AppConfig>,
    mut ask: impl FnMut(Question) -> Result<String, CliError>,
) -> Result<InitInputs, CliError> {
    let mut answer = |question: Question| -> Result<String, CliError> {
        let value = ask(question)?.trim().to_string();
        if value.is_empty() {
            let (field, flag) = question.field_and_flag();
            return Err(CliError::EmptyInput { field, flag });
        }
        Ok(value)
    };

    let instance_url = match instance_url {
        Some(url) => Some(url.to_string()),
        None if existing.is_some() => None,
        None => Some(answer(Question::InstanceUrl)?),
    };
    let service_user = match key_file {
        Some(path) => Some(read_key_file(path)?),
        None if mode == InitMode::Service => Some(parse_pasted_key(&answer(Question::ServiceUserKey)?)?),
        None => None,
    };
    let client_id = match client_id {
        Some(id) => Some(id.to_string()),
        None if mode == InitMode::Service => None,
        None => Some(answer(Question::ClientId)?),
    };
    Ok(InitInputs { instance_url, service_user, client_id })
}

/// Validates a pasted key exactly like `--key-file` does.
fn parse_pasted_key(raw: &str) -> Result<ServiceUserKey, CliError> {
    ServiceUserKey::from_key_file(raw).map_err(|e| CliError::InvalidPastedKey { reason: e.to_string() })
}

/// Asks `question` on the terminal: the key hidden (no echo) when stdin is a
/// terminal, everything else as a visible line.
fn ask_on_stdin(question: Question) -> Result<String, CliError> {
    match question {
        Question::ServiceUserKey => {
            read_secret_with(question.label(), io::stdin().is_terminal(), rpassword::prompt_password, prompt)
        }
        Question::InstanceUrl | Question::ClientId => prompt(question.label()),
    }
}

/// Prints `label` on stderr (stdout carries only JSON) and reads one line.
fn prompt(label: &str) -> Result<String, CliError> {
    eprint!("{label}: ");
    io::stderr().flush().map_err(|e| CliError::IoError { reason: e.to_string() })?;
    let line = io::stdin()
        .lock()
        .lines()
        .next()
        .ok_or_else(|| CliError::IoError { reason: format!("unexpected end of input while reading {label}") })?
        .map_err(|e| CliError::IoError { reason: e.to_string() })?;
    Ok(line.trim().to_string())
}

/// Reads a secret: hidden (no echo) when stdin is a terminal, so it never lands
/// in scrollback; a plain line otherwise, so piped input keeps working.
pub(crate) fn read_secret_with(
    label: &str,
    is_terminal: bool,
    hidden: impl FnOnce(String) -> io::Result<String>,
    line: impl FnOnce(&str) -> Result<String, CliError>,
) -> Result<String, CliError> {
    if !is_terminal {
        return line(label);
    }
    let value = hidden(format!("{label}: "))
        .map_err(|e| CliError::IoError { reason: format!("could not read {label}: {e}") })?;
    Ok(value.trim().to_string())
}

/// `doctor`'s `app_config` check, the only one `init --user-app` runs (no identity
/// to check yet), as `{"app_config": ...}`; not ok without a Native app client id,
/// which every person's login needs.
pub(crate) fn user_app_report(config_dir: &Path) -> (Value, bool) {
    let (mut check, config) = doctor::check_app_config(config_dir);
    let ok = match config {
        Some(config) if config.client_id.is_some() => true,
        Some(_) => {
            check["status"] = json!("error");
            check["message"] = json!(
                "no Native app configured: app.json has no \"client_id\". Run: zitadel init --user-app --client-id <client-id>"
            );
            false
        }
        None => false,
    };
    (json!({"app_config": check}), ok)
}

/// `init --user-app` names no person: refuses `--user <id>`.
pub fn check_user_app_flag(identity: &Identity) -> Result<(), CliError> {
    match identity {
        Identity::Service => Ok(()),
        Identity::User(id) => Err(CliError::UserAppWithUser { id: id.to_string() }),
    }
}

/// Runs `init --user-app`: writes `app.json` like `init`, logs nobody in, and
/// prints the `app_config` check.
pub fn run_init_user_app(
    identity: &Identity,
    instance_url: Option<&str>,
    key_file: Option<&Path>,
    client_id: Option<&str>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    check_user_app_flag(identity)?;
    eprintln!("{}", InitMode::UserApp.steps());
    let config_dir = config_dir()?;
    let existing = load_existing(&config_dir);
    let inputs = resolve_init_inputs(InitMode::UserApp, instance_url, key_file, client_id, existing.as_ref(), ask_on_stdin)?;
    write_config(&config_dir, existing, inputs)?;

    let (report, ok) = user_app_report(&config_dir);
    print_json(&report, select.or_all())?;
    if !ok {
        return Err(CliError::DoctorCheckFailed);
    }
    eprintln!(
        "Nobody is logged in. Log each person in with `zitadel auth login --user <USER_ID>` (browser on this \
        machine) or `zitadel auth login --user <USER_ID> --remote --redirect-uri <URL>` (the person opens the \
        link elsewhere)."
    );
    Ok(())
}

pub(crate) fn read_key_file(path: &Path) -> Result<ServiceUserKey, CliError> {
    let raw = std::fs::read_to_string(path).map_err(|e| CliError::KeyFileUnreadable {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    ServiceUserKey::from_key_file(&raw).map_err(|e| CliError::InvalidKeyFile {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
}

/// True when an existing config pointed at a different instance — its stored
/// token belongs to that other instance and must not be reused.
pub(crate) fn instance_changed(previous: Option<&AppConfig>, new: &AppConfig) -> bool {
    previous.is_some_and(|p| p.instance_url != new.instance_url)
}

/// Removes every stored login (the service user's and every person's): they
/// belong to the previous instance.
pub(crate) fn discard_instance_credentials(config_dir: &Path) -> Result<(), CliError> {
    let failed = |e: std::io::Error| CliError::SaveCredentialsFailed {
        path: config_dir.join("zitadel-cli").display().to_string(),
        reason: format!("could not remove the previous instance's credentials: {e}"),
    };
    let users = auth::list_users(config_dir).map_err(failed)?;
    for stale in std::iter::once(Identity::Service).chain(users.into_iter().map(Identity::User)) {
        auth::remove_identity(config_dir, &stale).map_err(failed)?;
    }
    Ok(())
}

/// Merges flags over an existing config; flags win, omitted flags keep the
/// existing value. The instance URL is validated/normalized by `AppConfig::from_json`.
pub(crate) fn build_app_config(
    existing: Option<AppConfig>,
    instance_url: Option<&str>,
    service_user: Option<ServiceUserKey>,
    client_id: Option<&str>,
) -> Result<AppConfig, CliError> {
    let (existing_url, existing_key, existing_client) = match existing {
        Some(c) => (Some(c.instance_url), c.service_user, c.client_id),
        None => (None, None, None),
    };
    let url = instance_url
        .map(str::to_string)
        .or(existing_url)
        .ok_or(CliError::InstanceUrlRequired)?;

    let value = app_config_json(&AppConfig {
        instance_url: url.clone(),
        service_user: service_user.or(existing_key),
        client_id: client_id.map(str::to_string).or(existing_client),
    });
    AppConfig::from_json(&value.to_string()).map_err(|e| match e {
        AppConfigError::InvalidInstanceUrl(_) => CliError::InvalidInstanceUrl { value: url },
        // The value was just built from typed fields; only the URL can be invalid.
        other => CliError::Internal {
            reason: format!("re-parsing the merged app config failed: {other}"),
        },
    })
}

fn app_config_json(config: &AppConfig) -> serde_json::Value {
    let mut value = json!({"instance_url": config.instance_url});
    if let Some(key) = &config.service_user {
        value["service_user"] = json!(key);
    }
    if let Some(client_id) = &config.client_id {
        value["client_id"] = json!(client_id);
    }
    value
}

/// Writes `app.json` atomically with owner-only permissions (it holds the
/// private key), creating its folder; a looser existing file ends up 0600 too.
pub(crate) fn write_app_config(path: &Path, config: &AppConfig) -> Result<(), CliError> {
    let fail = |e: std::io::Error| CliError::WriteAppConfigFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    };
    let mut contents = serde_json::to_string_pretty(&app_config_json(config)).map_err(|e| {
        CliError::WriteAppConfigFailed {
            path: path.display().to_string(),
            reason: e.to_string(),
        }
    })?;
    contents.push('\n');
    oauth_user_login::write_secret_file(path, contents.as_bytes()).map_err(fail)
}

#[cfg(test)]
#[path = "../tests/commands/init_tests.rs"]
mod tests;
