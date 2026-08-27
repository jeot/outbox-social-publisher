use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::Utc;
use clap::Parser;
use diesel::sqlite::SqliteConnection;
use diesel::RunQueryDsl;
use diesel::QueryableByName;
use diesel::sql_query;
use diesel::sql_types::{Integer, Nullable, Text};
use rand::Rng;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{oneshot, Mutex};
use url::Url;
use urlencoding::encode;
use uuid::Uuid;

mod api;
mod auth;
mod cli;
mod config;
mod db;
mod errors;
mod jobs;
mod publish;
mod util;

const DEFAULT_X_SCOPES: &str = "tweet.read tweet.write users.read media.write offline.access";

use auth::{env_non_empty, load_linkedin_auth, load_x_auth};
use cli::*;
use config::{RuntimeConfig, load_config};
#[cfg(test)]
use config::{RuntimePaths, SignatureLayer};
use db::{ensure_db_ready, open_db};
use errors::AppError;
use util::json::print_json;

#[derive(Debug, Serialize)]
struct SuccessOutput {
    ok: bool,
    platform: &'static str,
    post_id: Option<String>,
    post_url: Option<String>,
    request_id: Option<String>,
    published_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct OAuthState {
    state: String,
    created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct XOAuthState {
    state: String,
    code_verifier: String,
    client_id: String,
    redirect_uri: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct TokenExchangeResponse {
    access_token: String,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    refresh_token_expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    sub: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XTokenExchangeResponse {
    access_token: String,
    token_type: Option<String>,
    expires_in: Option<u64>,
    scope: Option<String>,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XMediaUploadResponse {
    data: Option<XMediaUploadData>,
    errors: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
struct XMediaUploadData {
    id: String,
}

#[derive(Debug, Deserialize)]
struct LinkedinImageInitResponse {
    value: LinkedinImageInitValue,
}

#[derive(Debug, Deserialize)]
struct LinkedinImageInitValue {
    #[serde(rename = "uploadUrl")]
    upload_url: String,
    image: String,
}

#[derive(Debug, Deserialize)]
struct XCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LinkedinCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug)]
struct XAuthResult {
    access_token_expires_in: Option<u64>,
    refresh_token_saved: bool,
    scope: Option<String>,
    token_type: Option<String>,
}

#[derive(Debug)]
struct LinkedinAuthResult {
    access_token_expires_in: Option<u64>,
    refresh_token_saved: bool,
    author_urn: String,
    profile_name: Option<String>,
}

#[derive(Debug)]
struct ParsedPostInput {
    publish_text: String,
    media_paths: Vec<PathBuf>,
    file_sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PublishLogEntry {
    platform: String,
    author_urn: String,
    file_path: String,
    fingerprint: String,
    #[serde(default, alias = "content_sha256")]
    file_sha256: String,
    #[serde(default)]
    text_sha256: String,
    #[serde(default)]
    post_id: Option<String>,
    #[serde(default)]
    post_url: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
    published_at: String,
}

#[derive(Debug, QueryableByName)]
struct JobRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    action_group_id: String,
    #[diesel(sql_type = Text)]
    content_group_id: String,
    #[diesel(sql_type = Text)]
    asset_id: String,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Nullable<Text>)]
    platform: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    publish_mode: Option<String>,
    #[diesel(sql_type = Text)]
    workspace_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    owner_user_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    operator: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    user_note: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    ai_note: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    ai_model: Option<String>,
    #[diesel(sql_type = Text)]
    tags: String,
    #[diesel(sql_type = Text)]
    selected_platforms: String,
    #[diesel(sql_type = Text)]
    file_path: String,
    #[diesel(sql_type = Nullable<Text>)]
    run_at_utc: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    timezone: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    status_reason: Option<String>,
    #[diesel(sql_type = Integer)]
    attempt_count: i32,
    #[diesel(sql_type = Nullable<Text>)]
    file_sha256: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    text_sha256: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    fingerprint: Option<String>,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
}

#[derive(Debug, QueryableByName)]
struct IdRow {
    #[diesel(sql_type = Text)]
    id: String,
}

#[derive(Debug, QueryableByName)]
struct AssetLinkRow {
    #[diesel(sql_type = Text)]
    asset_id: String,
    #[diesel(sql_type = Text)]
    content_group_id: String,
}


#[derive(Clone)]
struct XOAuthCallbackState {
    expected_state: String,
    code_verifier: String,
    client_id: String,
    client_secret: Option<String>,
    redirect_uri: String,
    connect_timeout: Duration,
    request_timeout: Duration,
    result_tx: Arc<Mutex<Option<oneshot::Sender<Result<XAuthResult, AppError>>>>>,
}

#[derive(Clone)]
struct LinkedinOAuthCallbackState {
    expected_state: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    connect_timeout: Duration,
    request_timeout: Duration,
    result_tx: Arc<Mutex<Option<oneshot::Sender<Result<LinkedinAuthResult, AppError>>>>>,
}

pub async fn run() -> ExitCode {
    let cli = Cli::parse();
    if let Commands::Init(args) = cli.command {
        match run_init(args) {
            Ok(value) => {
                print_json(&value, true);
                return ExitCode::from(0);
            }
            Err(err) => {
                print_json(&err.to_output(), true);
                return ExitCode::from(err.exit_code());
            }
        }
    }

    let config = match load_config() {
        Ok(config) => config,
        Err(err) => {
            print_json(&err.to_output(), true);
            return ExitCode::from(err.exit_code());
        }
    };
    let _ = dotenvy::from_filename_override(&config.paths.env_path);
    let requires_writable_db = matches!(&cli.command, Commands::Job(_) | Commands::Serve(_));
    if requires_writable_db {
        if let Err(err) = ensure_db_ready(&config) {
            print_json(&err.to_output(), config.pretty_json);
            return ExitCode::from(err.exit_code());
        }
    }

    if let Commands::Serve(args) = cli.command {
        if let Err(err) = validate_serve_requirements(&config) {
            print_json(&err.to_output(), config.pretty_json);
            return ExitCode::from(err.exit_code());
        }
        let host = args.host.unwrap_or_else(|| config.api_host.clone());
        let port = args.port.unwrap_or(config.api_port);
        return api::run_server(host, port, config).await;
    }

    let result: Result<Value, AppError> = match cli.command {
        Commands::Paths => show_paths(&config),
        Commands::Init(_) => unreachable!("init command handled before runtime config load"),
        Commands::Workspace(workspace) => match workspace.command {
            WorkspaceCommand::Switch(args) => workspace_switch(args),
        },
        Commands::Publish(publish) => match publish.platform {
            PublishPlatform::Linkedin(args) => publish_linkedin_cli(args, &config).await,
            PublishPlatform::X(args) => publish_x_cli(args, &config).await,
        },
        Commands::Job(job) => match job.command {
            JobCommand::Ready(args) => job_ready(args, &config),
            JobCommand::Unready(args) => job_unready(args, &config),
            JobCommand::Schedule(args) => job_schedule(args, &config),
            JobCommand::Unschedule(args) => job_unschedule(args, &config),
            JobCommand::AddSchedule(args) => job_add_schedule(args, &config),
            JobCommand::Cancel(args) => job_cancel(args, &config),
            JobCommand::List(args) => job_list(args, &config),
            JobCommand::Show(args) => job_show(args, &config),
            JobCommand::RunDebug(args) => job_run_debug(args, &config),
        },
        Commands::Worker(worker) => match worker.command {
            WorkerCommand::Run(args) => worker_run_dry_once(args, &config),
        },
        Commands::Auth(auth) => match auth.platform {
            AuthPlatform::Linkedin(args) => match args.command {
                AuthLinkedinCommand::Guide => show_linkedin_auth_guide(),
                AuthLinkedinCommand::Login => start_linkedin_login(&config).await,
                AuthLinkedinCommand::Exchange(args) => exchange_linkedin_code(args, &config).await,
                AuthLinkedinCommand::Whoami => resolve_linkedin_author_urn(&config).await,
                AuthLinkedinCommand::TokenStatus => show_linkedin_token_status(),
                AuthLinkedinCommand::TokenRefresh => run_linkedin_token_refresh(&config).await,
            },
            AuthPlatform::X(args) => match args.command {
                AuthXCommand::Login => start_x_login(&config).await,
                AuthXCommand::Exchange(args) => exchange_x_code_manual(args, &config).await,
                AuthXCommand::TokenStatus => show_x_token_status(),
                AuthXCommand::TokenRefresh => run_x_token_refresh(&config).await,
            },
        },
        Commands::Serve(_) => unreachable!("serve command handled before standard JSON dispatch"),
    };

    match result {
        Ok(value) => {
            print_json(&value, config.pretty_json);
            ExitCode::from(0)
        }
        Err(err) => {
            print_json(&err.to_output(), config.pretty_json);
            ExitCode::from(err.exit_code())
        }
    }
}

fn now_rfc3339_utc() -> String {
    Utc::now().to_rfc3339()
}

fn validate_publish_pass(passed: Option<&str>, config: &RuntimeConfig) -> Result<(), AppError> {
    let provided = passed.ok_or(AppError::Validation {
        message: "Missing publish password.".to_string(),
        suggestion: Some("Pass --pass <publish-pass> for real publish commands.".to_string()),
        command: None,
    })?;
    if provided == config.publish_cli_password {
        return Ok(());
    }
    Err(AppError::Validation {
        message: "Invalid publish password.".to_string(),
        suggestion: Some(
            "Pass the correct --pass value, or update [security].publish_cli_password in config."
                .to_string(),
        ),
        command: None,
    })
}

fn validate_serve_requirements(config: &RuntimeConfig) -> Result<(), AppError> {
    if !config.paths.global_config_path.exists() {
        return Err(AppError::Validation {
            message: format!(
                "Global config not found: {}",
                config.paths.global_config_path.display()
            ),
            suggestion: Some("Run `publo init` to create required files.".to_string()),
            command: Some("publo init".to_string()),
        });
    }

    if !config.paths.workspace_config_path.exists() {
        return Err(AppError::Validation {
            message: format!(
                "Workspace config not found for workspace '{}': {}",
                config.paths.workspace_key,
                config.paths.workspace_config_path.display()
            ),
            suggestion: Some("Run `publo init` to create required files.".to_string()),
            command: Some("publo init --workspace-id <id> --display-name \"My Workspace\"".to_string()),
        });
    }

    if config.catalog_roots.is_empty() {
        return Err(AppError::Validation {
            message: "No catalog roots configured.".to_string(),
            suggestion: Some(
                "Set [catalog].roots in workspace config.toml, then retry serve."
                    .to_string(),
            ),
            command: Some("publo paths".to_string()),
        });
    }

    for root in &config.catalog_roots {
        if !root.exists() {
            return Err(AppError::Validation {
                message: format!("Catalog root does not exist: {}", root.display()),
                suggestion: Some(
                    "Fix [catalog].roots paths in workspace config.toml and retry."
                        .to_string(),
                ),
                command: Some("publo paths".to_string()),
            });
        }
        if !root.is_dir() {
            return Err(AppError::Validation {
                message: format!("Catalog root is not a directory: {}", root.display()),
                suggestion: Some(
                    "Use directory paths in [catalog].roots and retry.".to_string(),
                ),
                command: Some("publo paths".to_string()),
            });
        }
    }

    Ok(())
}

fn show_paths(config: &RuntimeConfig) -> Result<Value, AppError> {
    Ok(json!({
        "ok": true,
        "mode": "paths",
        "publo_home": config.paths.publo_home.display().to_string(),
        "global_config_path": config.paths.global_config_path.display().to_string(),
        "global_config_exists": config.paths.global_config_path.exists(),
        "workspace_key": config.paths.workspace_key,
        "workspace_id": config.workspace_id,
        "workspace_display_name": config.workspace_display_name,
        "workspace_config_path": config.paths.workspace_config_path.display().to_string(),
        "workspace_config_exists": config.paths.workspace_config_path.exists(),
        "env_path": config.paths.env_path.display().to_string(),
        "env_exists": config.paths.env_path.exists(),
        "runtime_dir": config.paths.runtime_dir.display().to_string(),
        "workspace_dir": config.paths.workspace_dir.display().to_string(),
        "db_path": config.db_path.display().to_string(),
        "publish_log_path": config.paths.publish_log_path.display().to_string(),
        "linkedin_oauth_state_path": config.paths.linkedin_oauth_state_path.display().to_string(),
        "x_oauth_state_path": config.paths.x_oauth_state_path.display().to_string()
    }))
}

fn run_init(args: InitArgs) -> Result<Value, AppError> {
    let default_workspace_key = config::resolve_runtime_paths().workspace_key;
    let default_display_name = config::default_workspace_display_name(&default_workspace_key);

    let selected_display_name = if let Some(value) = args.display_name {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            default_display_name.clone()
        } else {
            trimmed
        }
    } else {
        prompt_workspace_display_name(&default_display_name)?
    };

    let selected_workspace_key = if let Some(value) = args.workspace_id {
        config::normalize_workspace_key(&value)
    } else {
        config::normalize_workspace_key(&selected_display_name)
    };

    let paths = config::resolve_runtime_paths_for_workspace(&selected_workspace_key);
    let workspace_id = generate_id();
    let mut steps: Vec<Value> = Vec::new();

    ensure_dir_step(&paths.publo_home, "publo_home", &mut steps)?;
    ensure_file_step(
        &paths.global_config_path,
        &global_config_template(&paths.workspace_key),
        "global_config",
        &mut steps,
    )?;
    ensure_dir_step(&paths.workspace_dir, "workspace_dir", &mut steps)?;
    ensure_file_step(
        &paths.workspace_config_path,
        &workspace_config_template(&workspace_id, &selected_display_name),
        "workspace_config",
        &mut steps,
    )?;
    let workspace_id = ensure_workspace_config_identity(
        &paths.workspace_config_path,
        &workspace_id,
        &mut steps,
    )?;
    ensure_file_step(
        &paths.workspace_dir.join(".env.example"),
        &workspace_env_example_template(),
        "workspace_env_example",
        &mut steps,
    )?;
    ensure_file_step(
        &paths.env_path,
        &workspace_env_template(),
        "workspace_env",
        &mut steps,
    )?;
    ensure_dir_step(&paths.runtime_dir, "workspace_runtime_dir", &mut steps)?;

    Ok(json!({
        "ok": true,
        "mode": "init",
        "workspace_key": paths.workspace_key,
        "workspace_id": workspace_id,
        "workspace_display_name": selected_display_name,
        "publo_home": paths.publo_home.display().to_string(),
        "global_config_path": paths.global_config_path.display().to_string(),
        "workspace_dir": paths.workspace_dir.display().to_string(),
        "workspace_config_path": paths.workspace_config_path.display().to_string(),
        "env_path": paths.env_path.display().to_string(),
        "steps": steps,
        "note": "Existing files were left untouched."
    }))
}

fn workspace_switch(args: WorkspaceSwitchArgs) -> Result<Value, AppError> {
    let workspace_key = config::normalize_workspace_key(&args.workspace_id);
    let paths = config::resolve_runtime_paths_for_workspace(&workspace_key);

    if !paths.workspace_config_path.exists() {
        return Err(AppError::Validation {
            message: format!(
                "Workspace '{}' does not exist. Expected config at {}",
                workspace_key,
                paths.workspace_config_path.display()
            ),
            suggestion: Some(
                "Create it first with `publo init --workspace-id <id> --display-name \"Name\"`."
                    .to_string(),
            ),
            command: Some(
                "publo init --workspace-id <id> --display-name \"Name\"".to_string(),
            ),
        });
    }

    let global_config_path = paths.global_config_path.clone();
    if !global_config_path.exists() {
        return Err(AppError::Validation {
            message: format!(
                "Global config not found: {}",
                global_config_path.display()
            ),
            suggestion: Some("Run `publo init` first.".to_string()),
            command: Some("publo init".to_string()),
        });
    }

    let current_raw = fs::read_to_string(&global_config_path).map_err(|err| AppError::Io {
        message: format!(
            "Failed to read global config '{}': {err}",
            global_config_path.display()
        ),
    })?;
    let updated = set_default_workspace_in_global_config(&current_raw, &workspace_key);
    fs::write(&global_config_path, updated).map_err(|err| AppError::Io {
        message: format!(
            "Failed to update global config '{}': {err}",
            global_config_path.display()
        ),
    })?;

    Ok(json!({
        "ok": true,
        "mode": "workspace_switch",
        "workspace_key": workspace_key,
        "global_config_path": global_config_path.display().to_string(),
        "note": "Default workspace updated."
    }))
}

fn prompt_workspace_display_name(default_workspace_display_name: &str) -> Result<String, AppError> {
    print!(
        "Workspace display name [{}]: ",
        default_workspace_display_name
    );
    io::stdout().flush().map_err(|err| AppError::Io {
        message: format!("Failed to flush stdout for workspace prompt: {err}"),
    })?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|err| AppError::Io {
        message: format!("Failed to read workspace name from stdin: {err}"),
    })?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(default_workspace_display_name.to_string());
    }
    Ok(trimmed.to_string())
}

fn ensure_dir_step(path: &PathBuf, key: &str, steps: &mut Vec<Value>) -> Result<(), AppError> {
    if path.exists() {
        steps.push(json!({
            "key": key,
            "path": path.display().to_string(),
            "status": "exists",
            "touched": false
        }));
        return Ok(());
    }
    fs::create_dir_all(path).map_err(|err| AppError::Io {
        message: format!("Failed to create directory '{}': {err}", path.display()),
    })?;
    steps.push(json!({
        "key": key,
        "path": path.display().to_string(),
        "status": "created",
        "touched": true
    }));
    Ok(())
}

fn ensure_file_step(path: &PathBuf, content: &str, key: &str, steps: &mut Vec<Value>) -> Result<(), AppError> {
    if path.exists() {
        steps.push(json!({
            "key": key,
            "path": path.display().to_string(),
            "status": "exists",
            "touched": false
        }));
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| AppError::Io {
            message: format!("Failed to create directory '{}': {err}", parent.display()),
        })?;
    }

    fs::write(path, content).map_err(|err| AppError::Io {
        message: format!("Failed to create file '{}': {err}", path.display()),
    })?;
    steps.push(json!({
        "key": key,
        "path": path.display().to_string(),
        "status": "created",
        "touched": true
    }));
    Ok(())
}

fn ensure_workspace_config_identity(
    path: &PathBuf,
    generated_id: &str,
    steps: &mut Vec<Value>,
) -> Result<String, AppError> {
    let raw = fs::read_to_string(path).map_err(|err| AppError::Io {
        message: format!("Failed to read workspace config '{}': {err}", path.display()),
    })?;
    if let Some(existing) = raw.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "id").then(|| value.trim().trim_matches('"').to_string())
    }) {
        Uuid::parse_str(&existing).map_err(|_| AppError::Validation {
            message: format!("Workspace config has invalid [workspace].id: {}", path.display()),
            suggestion: Some("Set [workspace].id to a UUID v4.".to_string()),
            command: None,
        })?;
        return Ok(existing);
    }

    let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();
    let workspace_index = lines.iter().position(|line| line.trim() == "[workspace]").ok_or_else(|| {
        AppError::Validation {
            message: format!("Workspace config has no [workspace] section: {}", path.display()),
            suggestion: Some("Add a [workspace] section and run `publo init` again.".to_string()),
            command: None,
        }
    })?;
    lines.insert(workspace_index + 1, format!("id = \"{generated_id}\""));
    fs::write(path, format!("{}\n", lines.join("\n"))).map_err(|err| AppError::Io {
        message: format!("Failed to write workspace config '{}': {err}", path.display()),
    })?;
    steps.push(json!({
        "key": "workspace_identity",
        "path": path.display().to_string(),
        "status": "created",
        "touched": true
    }));
    Ok(generated_id.to_string())
}

fn global_config_template(default_workspace_id: &str) -> String {
    let base = include_str!("../.publo.so.example/config.toml").to_string();
    if default_workspace_id == "default" {
        return base;
    }
    base.replace(
        "default = \"default\"",
        &format!("default = \"{}\"", default_workspace_id),
    )
}

fn workspace_config_template(workspace_id: &str, display_name: &str) -> String {
    let base = include_str!("../.publo.so.example/workspaces/default/config.toml").to_string();
    base.replace(
        "[workspace]\n",
        &format!("[workspace]\nid = \"{workspace_id}\"\n"),
    ).replace(
        "display_name = \"Default\"",
        &format!("display_name = \"{}\"", display_name.replace('"', "\\\"")),
    )
}

fn workspace_env_example_template() -> String {
    include_str!("../.publo.so.example/workspaces/default/.env.example").to_string()
}

fn workspace_env_template() -> String {
    workspace_env_example_template()
}

fn set_default_workspace_in_global_config(raw: &str, workspace_id: &str) -> String {
    let mut lines: Vec<String> = raw.lines().map(|s| s.to_string()).collect();
    let mut workspace_header_idx: Option<usize> = None;
    let mut default_idx: Option<usize> = None;
    let mut workspace_section_end = lines.len();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == "[workspace]" {
            workspace_header_idx = Some(idx);
            workspace_section_end = lines.len();
            for (next_idx, next_line) in lines.iter().enumerate().skip(idx + 1) {
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with('[') && next_trimmed.ends_with(']') {
                    workspace_section_end = next_idx;
                    break;
                }
                if next_trimmed.starts_with("default") && next_trimmed.contains('=') {
                    default_idx = Some(next_idx);
                }
            }
            break;
        }
    }

    let new_default_line = format!("default = \"{}\"", workspace_id);
    match (workspace_header_idx, default_idx) {
        (Some(_), Some(idx)) => {
            lines[idx] = new_default_line;
        }
        (Some(header_idx), None) => {
            let insert_at = (header_idx + 1).min(workspace_section_end);
            lines.insert(insert_at, new_default_line);
        }
        (None, _) => {
            if !lines.is_empty() && !lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[workspace]".to_string());
            lines.push(new_default_line);
        }
    }

    let mut out = lines.join("\n");
    if raw.ends_with('\n') || out.is_empty() {
        out.push('\n');
    }
    out
}

fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

fn parse_run_at_to_utc(input: &str) -> Result<String, AppError> {
    let dt = chrono::DateTime::parse_from_rfc3339(input).map_err(|err| AppError::Validation {
        message: format!("Invalid --at datetime. Expected RFC3339. Parse error: {err}"),
        suggestion: Some(
            "Use format like 2026-08-21T10:30:00+03:30 or 2026-08-21T07:00:00Z.".to_string(),
        ),
        command: None,
    })?;
    Ok(dt.with_timezone(&Utc).to_rfc3339())
}

fn validate_operator_notes(
    operator: OperatorArg,
    ai_note: &Option<String>,
    ai_model: &Option<String>,
) -> Result<(), AppError> {
    if matches!(operator, OperatorArg::User) && (ai_note.is_some() || ai_model.is_some()) {
        return Err(AppError::Validation {
            message: "--ai-note/--ai-model require --by ai.".to_string(),
            suggestion: Some("Use --by ai when providing AI metadata.".to_string()),
            command: None,
        });
    }
    Ok(())
}

fn selected_platforms_json(platforms: &[PlatformArg]) -> String {
    let values: Vec<&str> = platforms.iter().map(|platform| platform.as_str()).collect();
    serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_string())
}

fn selected_platforms_from_json(raw: &str) -> Vec<String> {
    let values: Vec<String> = serde_json::from_str(raw).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for value in values {
        let normalized = match value.as_str() {
            "linkedin" => Some("linkedin"),
            "x" => Some("x"),
            _ => None,
        };
        if let Some(platform) = normalized
            && !out.iter().any(|item| item == platform)
        {
            out.push(platform.to_string());
        }
    }
    out
}

fn resolve_asset_link_for_paths(
    conn: &mut SqliteConnection,
    canonical_path: &str,
    requested_path: &str,
) -> Result<Option<AssetLinkRow>, AppError> {
    let mut rows: Vec<AssetLinkRow> = sql_query(
        "SELECT asset_id, content_group_id
         FROM jobs
         WHERE deleted_at IS NULL
           AND (file_path = ? OR file_path = ?)
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1",
    )
    .bind::<Text, _>(canonical_path)
    .bind::<Text, _>(requested_path)
    .load(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to resolve existing asset linkage: {err}"),
    })?;
    Ok(rows.pop())
}

fn find_ready_intent_id_for_asset(
    conn: &mut SqliteConnection,
    asset_id: &str,
) -> Result<Option<String>, AppError> {
    let mut rows: Vec<IdRow> = sql_query(
        "SELECT id
         FROM jobs
         WHERE asset_id = ?
           AND status = 'ready'
           AND platform IS NULL
           AND deleted_at IS NULL
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1",
    )
    .bind::<Text, _>(asset_id)
    .load(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to load ready intent job: {err}"),
    })?;
    Ok(rows.pop().map(|row| row.id))
}

fn find_job_id_by_asset_platform(
    conn: &mut SqliteConnection,
    asset_id: &str,
    platform: &str,
) -> Result<Option<String>, AppError> {
    let mut rows: Vec<IdRow> = sql_query(
        "SELECT id
         FROM jobs
         WHERE asset_id = ?
           AND platform = ?
           AND deleted_at IS NULL
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1",
    )
    .bind::<Text, _>(asset_id)
    .bind::<Text, _>(platform)
    .load(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to load existing job by asset+platform: {err}"),
    })?;
    Ok(rows.pop().map(|row| row.id))
}

fn job_ready(args: JobReadyArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    validate_operator_notes(args.by, &args.ai_note, &args.ai_model)?;
    let mut selected_platforms: Vec<PlatformArg> = Vec::new();
    for platform in &args.platform {
        if !selected_platforms.contains(platform) {
            selected_platforms.push(*platform);
        }
    }
    let file_raw = fs::read_to_string(&args.file).map_err(|err| AppError::Io {
        message: format!("Failed to read post file {}: {err}", args.file.display()),
    })?;
    let publish_text = publish::extract_publish_text(&file_raw);
    let text_sha256 = compute_content_sha256(publish_text.as_bytes());
    let file_sha256 = compute_content_sha256(file_raw.as_bytes());
    let file_path = fs::canonicalize(&args.file).unwrap_or(args.file.clone());
    let file_path_str = file_path.display().to_string();
    let requested_path_str = args.file.display().to_string();
    let selected_platforms_json = selected_platforms_json(selected_platforms.as_slice());
    let run_at_utc = match args.at.as_deref() {
        Some(value) => Some(parse_run_at_to_utc(value)?),
        None => None,
    };
    let workspace_id = config.workspace_id.clone();

    let mut conn = open_db(config)?;
    let now = now_rfc3339_utc();
    let action_group_id = generate_id();
    let existing_link = resolve_asset_link_for_paths(&mut conn, &file_path_str, &requested_path_str)?;
    let asset_id = existing_link
        .as_ref()
        .map(|row| row.asset_id.clone())
        .unwrap_or_else(generate_id);
    let content_group_id = existing_link
        .as_ref()
        .map(|row| row.content_group_id.clone())
        .unwrap_or_else(generate_id);

    let mut created = false;
    let job_id = if let Some(existing_ready_id) = find_ready_intent_id_for_asset(&mut conn, &asset_id)? {
        sql_query(
            "UPDATE jobs
             SET action_group_id = ?,
                 content_group_id = ?,
                 kind = 'catalog',
                 status = 'ready',
                 platform = NULL,
                 publish_mode = NULL,
                 workspace_id = ?,
                 owner_user_id = ?,
                 operator = ?,
                 user_note = ?,
                 ai_note = ?,
                 ai_model = ?,
                 selected_platforms = ?,
                 file_path = ?,
                 run_at_utc = ?,
                 timezone = ?,
                 status_reason = NULL,
                 attempt_count = 0,
                 file_sha256 = ?,
                 text_sha256 = ?,
                 fingerprint = NULL,
                 updated_at = ?,
                 version = version + 1,
                 synced_at = NULL,
                 modified_by = 'local'
             WHERE id = ?",
        )
        .bind::<Text, _>(&action_group_id)
        .bind::<Text, _>(&content_group_id)
        .bind::<Text, _>(&workspace_id)
        .bind::<Nullable<Text>, _>(args.owner_user_id.as_deref())
        .bind::<Text, _>(args.by.as_str())
        .bind::<Nullable<Text>, _>(args.user_note.as_deref())
        .bind::<Nullable<Text>, _>(args.ai_note.as_deref())
        .bind::<Nullable<Text>, _>(args.ai_model.as_deref())
        .bind::<Text, _>(&selected_platforms_json)
        .bind::<Text, _>(&file_path_str)
        .bind::<Nullable<Text>, _>(run_at_utc.as_deref())
        .bind::<Nullable<Text>, _>(args.timezone.as_deref())
        .bind::<Text, _>(&file_sha256)
        .bind::<Text, _>(&text_sha256)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&existing_ready_id)
        .execute(&mut conn)
        .map_err(|err| AppError::Io {
            message: format!("Failed to update ready job: {err}"),
        })?;
        existing_ready_id
    } else {
        let id = generate_id();
        sql_query(
            "INSERT INTO jobs (
                id, action_group_id, content_group_id, asset_id, kind, status, platform, publish_mode, workspace_id, owner_user_id, operator, user_note, ai_note, ai_model, selected_platforms,
                file_path, run_at_utc, timezone, status_reason, attempt_count, file_sha256, text_sha256, fingerprint,
                created_at, updated_at, deleted_at, version, synced_at, modified_by
             ) VALUES (
                ?, ?, ?, ?, 'catalog', 'ready', NULL, NULL, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, NULL, 0, ?, ?, NULL,
                ?, ?, NULL, 1, NULL, 'local'
             )",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(&action_group_id)
        .bind::<Text, _>(&content_group_id)
        .bind::<Text, _>(&asset_id)
        .bind::<Text, _>(&workspace_id)
        .bind::<Nullable<Text>, _>(args.owner_user_id.as_deref())
        .bind::<Text, _>(args.by.as_str())
        .bind::<Nullable<Text>, _>(args.user_note.as_deref())
        .bind::<Nullable<Text>, _>(args.ai_note.as_deref())
        .bind::<Nullable<Text>, _>(args.ai_model.as_deref())
        .bind::<Text, _>(&selected_platforms_json)
        .bind::<Text, _>(&file_path_str)
        .bind::<Nullable<Text>, _>(run_at_utc.as_deref())
        .bind::<Nullable<Text>, _>(args.timezone.as_deref())
        .bind::<Text, _>(&file_sha256)
        .bind::<Text, _>(&text_sha256)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&now)
        .execute(&mut conn)
        .map_err(|err| AppError::Io {
            message: format!("Failed to insert ready job: {err}"),
        })?;
        created = true;
        id
    };

    Ok(json!({
        "ok": true,
        "mode": "job_ready",
        "job_id": job_id,
        "created": created,
        "status": "ready",
        "action_group_id": action_group_id,
        "content_group_id": content_group_id,
        "asset_id": asset_id,
        "platform": Value::Null,
        "selected_platforms": selected_platforms.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        "publish_mode": Value::Null,
        "run_at_utc": run_at_utc,
        "timezone": args.timezone,
        "file_path": file_path_str,
        "workspace_id": workspace_id,
        "operator": args.by.as_str(),
        "file_sha256": file_sha256,
        "text_sha256": text_sha256
    }))
}

pub(crate) fn job_unready(args: JobIdArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    let mut conn = open_db(config)?;
    let deleted = sql_query("DELETE FROM jobs WHERE id = ? AND status IN ('ready', 'blocked', 'canceled', 'disabled')")
        .bind::<Text, _>(&args.id)
        .execute(&mut conn)
        .map_err(|err| AppError::Io {
            message: format!("Failed to remove job: {err}"),
        })?;

    if deleted == 0 {
        return Err(AppError::Validation {
            message: "No removable job found. Only ready/blocked/canceled can be unready-removed."
                .to_string(),
            suggestion: Some("Use job unschedule or job cancel first when status is scheduled.".to_string()),
            command: None,
        });
    }

    Ok(json!({
        "ok": true,
        "mode": "job_unready",
        "job_id": args.id,
        "removed": true
    }))
}

pub(crate) fn job_schedule(args: JobScheduleArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    validate_operator_notes(args.by, &args.ai_note, &args.ai_model)?;
    let mut conn = open_db(config)?;
    let mut job = get_job_by_id(&mut conn, &args.id)?;
    let run_at_utc = parse_run_at_to_utc(&args.at)?;
    let timezone = args.timezone.clone();
    let selected_from_decision = selected_platforms_from_json(&job.selected_platforms);
    let selected_platform = match (args.platform, job.platform.as_deref()) {
        (Some(p), _) => p.as_str().to_string(),
        (None, Some(existing)) => existing.to_string(),
        (None, None) if selected_from_decision.len() == 1 => selected_from_decision[0].clone(),
        (None, None) if selected_from_decision.len() > 1 => {
            return Err(AppError::Validation {
                message: "This decision has multiple platforms. Pass --platform when scheduling."
                    .to_string(),
                suggestion: Some("Use --platform linkedin or --platform x.".to_string()),
                command: None,
            })
        }
        (None, None) => {
            return Err(AppError::Validation {
                message: "This ready job has no platform. Pass --platform when scheduling."
                    .to_string(),
                suggestion: Some("Use --platform linkedin or --platform x.".to_string()),
                command: None,
            })
        }
    };
    job.platform = Some(selected_platform);
    job.publish_mode = match job.platform.as_deref() {
        Some("x") => Some("single".to_string()),
        _ => None,
    };

    match preflight_job(&job, config) {
        Ok(preflight) => {
            let now = now_rfc3339_utc();
            sql_query(
                "UPDATE jobs
                 SET status = 'scheduled',
                     platform = ?,
                     publish_mode = ?,
                     run_at_utc = ?,
                     timezone = ?,
                     status_reason = NULL,
                     operator = ?,
                     user_note = COALESCE(?, user_note),
                     ai_note = COALESCE(?, ai_note),
                     ai_model = COALESCE(?, ai_model),
                     file_sha256 = ?,
                     text_sha256 = ?,
                     fingerprint = ?,
                     updated_at = ?,
                     version = version + 1,
                     synced_at = NULL,
                     modified_by = 'local'
                 WHERE id = ?",
            )
            .bind::<Nullable<Text>, _>(job.platform.as_deref())
            .bind::<Nullable<Text>, _>(job.publish_mode.as_deref())
            .bind::<Text, _>(&run_at_utc)
            .bind::<Nullable<Text>, _>(timezone.as_deref())
            .bind::<Text, _>(args.by.as_str())
            .bind::<Nullable<Text>, _>(args.user_note.as_deref())
            .bind::<Nullable<Text>, _>(args.ai_note.as_deref())
            .bind::<Nullable<Text>, _>(args.ai_model.as_deref())
            .bind::<Text, _>(&preflight.file_sha256)
            .bind::<Text, _>(&preflight.text_sha256)
            .bind::<Text, _>(&preflight.fingerprint)
            .bind::<Text, _>(&now)
            .bind::<Text, _>(&args.id)
            .execute(&mut conn)
            .map_err(|err| AppError::Io {
                message: format!("Failed to schedule job: {err}"),
            })?;

            job = get_job_by_id(&mut conn, &args.id)?;
            Ok(json!({
                "ok": true,
                "mode": "job_schedule",
                "job": job_to_json(&job),
                "preflight": preflight.details
            }))
        }
        Err(err) => {
            let now = now_rfc3339_utc();
            let reason = err.to_output().message;
            sql_query(
                "UPDATE jobs
                 SET status = 'blocked',
                     platform = ?,
                     publish_mode = ?,
                     status_reason = ?,
                     operator = ?,
                     user_note = COALESCE(?, user_note),
                     ai_note = COALESCE(?, ai_note),
                     ai_model = COALESCE(?, ai_model),
                     updated_at = ?,
                     version = version + 1,
                     synced_at = NULL,
                     modified_by = 'local'
                 WHERE id = ?",
            )
            .bind::<Nullable<Text>, _>(job.platform.as_deref())
            .bind::<Nullable<Text>, _>(job.publish_mode.as_deref())
            .bind::<Text, _>(&reason)
            .bind::<Text, _>(args.by.as_str())
            .bind::<Nullable<Text>, _>(args.user_note.as_deref())
            .bind::<Nullable<Text>, _>(args.ai_note.as_deref())
            .bind::<Nullable<Text>, _>(args.ai_model.as_deref())
            .bind::<Text, _>(&now)
            .bind::<Text, _>(&args.id)
            .execute(&mut conn)
            .map_err(|update_err| AppError::Io {
                message: format!("Failed to mark job as blocked after schedule preflight failure: {update_err}"),
            })?;

            Ok(json!({
                "ok": false,
                "mode": "job_schedule",
                "status": "blocked",
                "job_id": args.id,
                "reason": reason,
                "error": err.to_output()
            }))
        }
    }
}

pub(crate) fn job_unschedule(args: JobUnscheduleArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    let mut conn = open_db(config)?;
    let now = now_rfc3339_utc();
    let reason = args.reason.unwrap_or_else(|| "unscheduled manually".to_string());
    let changed = sql_query(
        "UPDATE jobs
         SET status='ready',
             status_reason=?,
             updated_at=?,
             version=version+1,
             synced_at=NULL,
             modified_by='local'
         WHERE id=? AND status='scheduled'",
    )
    .bind::<Text, _>(&reason)
    .bind::<Text, _>(&now)
    .bind::<Text, _>(&args.id)
    .execute(&mut conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to unschedule job: {err}"),
    })?;
    if changed == 0 {
        return Err(AppError::Validation {
            message: "Only scheduled jobs can be unscheduled.".to_string(),
            suggestion: None,
            command: None,
        });
    }
    let job = get_job_by_id(&mut conn, &args.id)?;
    Ok(json!({"ok": true, "mode": "job_unschedule", "job": job_to_json(&job)}))
}

pub(crate) fn job_add_schedule(args: JobAddScheduleArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    validate_operator_notes(args.by, &args.ai_note, &args.ai_model)?;
    let run_at_utc = parse_run_at_to_utc(&args.at)?;
    let file_path = fs::canonicalize(&args.file).unwrap_or(args.file.clone());
    let platform = args.platform.as_str();
    let workspace_id = config.workspace_id.clone();
    let selected_platforms_json = selected_platforms_json(&[args.platform]);

    let preflight_input = JobRow {
        id: String::new(),
        action_group_id: String::new(),
        content_group_id: String::new(),
        asset_id: String::new(),
        kind: "catalog".to_string(),
        status: "ready".to_string(),
        platform: Some(platform.to_string()),
        publish_mode: publish_mode_for_platform(args.platform).map(str::to_string),
        workspace_id: workspace_id.clone(),
        owner_user_id: args.owner_user_id.clone(),
        operator: Some(args.by.as_str().to_string()),
        user_note: args.user_note.clone(),
        ai_note: args.ai_note.clone(),
        ai_model: args.ai_model.clone(),
        tags: "[]".to_string(),
        selected_platforms: selected_platforms_json.clone(),
        file_path: file_path.display().to_string(),
        run_at_utc: None,
        timezone: None,
        status_reason: None,
        attempt_count: 0,
        file_sha256: None,
        text_sha256: None,
        fingerprint: None,
        created_at: String::new(),
        updated_at: String::new(),
    };

    match preflight_job(&preflight_input, config) {
        Ok(preflight) => {
            let mut conn = open_db(config)?;
            let now = now_rfc3339_utc();
            let action_group_id = generate_id();
            let file_path_str = file_path.display().to_string();
            let requested_path_str = args.file.display().to_string();
            let existing_link =
                resolve_asset_link_for_paths(&mut conn, &file_path_str, &requested_path_str)?;
            let asset_id = existing_link
                .as_ref()
                .map(|row| row.asset_id.clone())
                .unwrap_or_else(generate_id);
            let content_group_id = existing_link
                .as_ref()
                .map(|row| row.content_group_id.clone())
                .unwrap_or_else(generate_id);

            let changed = sql_query(
                "UPDATE jobs
                 SET action_group_id = ?,
                     content_group_id = ?,
                     kind = 'catalog',
                     status = 'scheduled',
                     platform = ?,
                     publish_mode = ?,
                     workspace_id = ?,
                     owner_user_id = ?,
                     operator = ?,
                     user_note = ?,
                     ai_note = ?,
                     ai_model = ?,
                     selected_platforms = ?,
                     file_path = ?,
                     run_at_utc = ?,
                     timezone = ?,
                     status_reason = NULL,
                     attempt_count = 0,
                     file_sha256 = ?,
                     text_sha256 = ?,
                     fingerprint = ?,
                     updated_at = ?,
                     version = version + 1,
                     synced_at = NULL,
                     modified_by = 'local'
                 WHERE asset_id = ?
                   AND platform = ?
                   AND deleted_at IS NULL",
            )
            .bind::<Text, _>(&action_group_id)
            .bind::<Text, _>(&content_group_id)
            .bind::<Text, _>(platform)
            .bind::<Nullable<Text>, _>(publish_mode_for_platform(args.platform))
            .bind::<Text, _>(&workspace_id)
            .bind::<Nullable<Text>, _>(args.owner_user_id.as_deref())
            .bind::<Text, _>(args.by.as_str())
            .bind::<Nullable<Text>, _>(args.user_note.as_deref())
            .bind::<Nullable<Text>, _>(args.ai_note.as_deref())
            .bind::<Nullable<Text>, _>(args.ai_model.as_deref())
            .bind::<Text, _>(&selected_platforms_json)
            .bind::<Text, _>(&file_path_str)
            .bind::<Text, _>(&run_at_utc)
            .bind::<Nullable<Text>, _>(args.timezone.as_deref())
            .bind::<Text, _>(&preflight.file_sha256)
            .bind::<Text, _>(&preflight.text_sha256)
            .bind::<Text, _>(&preflight.fingerprint)
            .bind::<Text, _>(&now)
            .bind::<Text, _>(&asset_id)
            .bind::<Text, _>(platform)
            .execute(&mut conn)
            .map_err(|err| AppError::Io {
                message: format!("Failed to update scheduled job: {err}"),
            })?;

            let job_id = if changed > 0 {
                find_job_id_by_asset_platform(&mut conn, &asset_id, platform)?
                    .ok_or_else(|| AppError::Io {
                        message: "Updated scheduled job but could not read it back.".to_string(),
                    })?
            } else {
                let id = generate_id();
                sql_query(
                    "INSERT INTO jobs (
                        id, action_group_id, content_group_id, asset_id, kind, status, platform, publish_mode, workspace_id, owner_user_id, operator, user_note, ai_note, ai_model, selected_platforms,
                        file_path, run_at_utc, timezone, status_reason, attempt_count, file_sha256, text_sha256, fingerprint,
                        created_at, updated_at, deleted_at, version, synced_at, modified_by
                     ) VALUES (
                        ?, ?, ?, ?, 'catalog', 'scheduled', ?, ?, ?, ?, ?, ?, ?, ?,
                        ?, ?, ?, NULL, 0, ?, ?, ?,
                        ?, ?, NULL, 1, NULL, 'local'
                     )",
                )
                .bind::<Text, _>(&id)
                .bind::<Text, _>(&action_group_id)
                .bind::<Text, _>(&content_group_id)
                .bind::<Text, _>(&asset_id)
                .bind::<Text, _>(platform)
                .bind::<Nullable<Text>, _>(publish_mode_for_platform(args.platform))
                .bind::<Text, _>(&workspace_id)
                .bind::<Nullable<Text>, _>(args.owner_user_id.as_deref())
                .bind::<Text, _>(args.by.as_str())
                .bind::<Nullable<Text>, _>(args.user_note.as_deref())
                .bind::<Nullable<Text>, _>(args.ai_note.as_deref())
                .bind::<Nullable<Text>, _>(args.ai_model.as_deref())
                .bind::<Text, _>(&selected_platforms_json)
                .bind::<Text, _>(&file_path_str)
                .bind::<Text, _>(&run_at_utc)
                .bind::<Nullable<Text>, _>(args.timezone.as_deref())
                .bind::<Text, _>(&preflight.file_sha256)
                .bind::<Text, _>(&preflight.text_sha256)
                .bind::<Text, _>(&preflight.fingerprint)
                .bind::<Text, _>(&now)
                .bind::<Text, _>(&now)
                .execute(&mut conn)
                .map_err(|err| AppError::Io {
                    message: format!("Failed to add scheduled job: {err}"),
                })?;
                id
            };
            let job = get_job_by_id(&mut conn, &job_id)?;
            Ok(json!({
                "ok": true,
                "mode": "job_add_schedule",
                "created": changed == 0,
                "job": job_to_json(&job),
                "preflight": preflight.details
            }))
        }
        Err(err) => {
            let reason = err.to_output().message;
            Ok(json!({
                "ok": false,
                "mode": "job_add_schedule",
                "job": Value::Null,
                "reason": reason,
                "error": err.to_output()
            }))
        }
    }
}

pub(crate) fn job_cancel(args: JobCancelArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    let mut conn = open_db(config)?;
    let now = now_rfc3339_utc();
    let reason = args.reason.unwrap_or_else(|| "canceled manually".to_string());
    let changed = sql_query(
        "UPDATE jobs
         SET status='canceled',
             status_reason=?,
             updated_at=?,
             version=version+1,
             synced_at=NULL,
             modified_by='local'
         WHERE id=? AND status IN ('scheduled','blocked','failed','disabled')",
    )
    .bind::<Text, _>(&reason)
    .bind::<Text, _>(&now)
    .bind::<Text, _>(&args.id)
    .execute(&mut conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to cancel job: {err}"),
    })?;
    if changed == 0 {
        return Err(AppError::Validation {
            message: "No cancelable job found for this id.".to_string(),
            suggestion: None,
            command: None,
        });
    }
    let job = get_job_by_id(&mut conn, &args.id)?;
    Ok(json!({"ok": true, "mode": "job_cancel", "job": job_to_json(&job)}))
}

fn job_list(args: JobListArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    if let Some(status) = args.status.as_deref()
        && !matches!(
            status,
            "ready" | "scheduled" | "publishing" | "published" | "failed" | "blocked" | "canceled" | "disabled"
        )
    {
        return Err(AppError::Validation {
            message: format!("Invalid --status value: {status}"),
            suggestion: Some(
                "Use one of: ready, scheduled, publishing, published, failed, blocked, canceled, disabled."
                    .to_string(),
            ),
            command: None,
        });
    }
    if args.limit == 0 {
        return Err(AppError::Validation {
            message: "--limit must be >= 1.".to_string(),
            suggestion: Some("Use --limit 1 or greater.".to_string()),
            command: None,
        });
    }
    let mut conn = open_db(config)?;
    let rows: Vec<JobRow> = match (args.status.as_ref(), args.platform) {
        (Some(status), Some(platform)) => sql_query(
            "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
             FROM jobs
             WHERE status = ? AND platform = ?
             ORDER BY COALESCE(run_at_utc, created_at) ASC
             LIMIT ?",
        )
        .bind::<Text, _>(status)
        .bind::<Text, _>(platform.as_str())
        .bind::<Integer, _>(args.limit as i32)
        .load(&mut conn),
        (Some(status), None) => sql_query(
            "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
             FROM jobs
             WHERE status = ?
             ORDER BY COALESCE(run_at_utc, created_at) ASC
             LIMIT ?",
        )
        .bind::<Text, _>(status)
        .bind::<Integer, _>(args.limit as i32)
        .load(&mut conn),
        (None, Some(platform)) => sql_query(
            "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
             FROM jobs
             WHERE platform = ?
             ORDER BY COALESCE(run_at_utc, created_at) ASC
             LIMIT ?",
        )
        .bind::<Text, _>(platform.as_str())
        .bind::<Integer, _>(args.limit as i32)
        .load(&mut conn),
        (None, None) => sql_query(
            "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
             FROM jobs
             ORDER BY COALESCE(run_at_utc, created_at) ASC
             LIMIT ?",
        )
        .bind::<Integer, _>(args.limit as i32)
        .load(&mut conn),
    }
    .map_err(|err| AppError::Io {
        message: format!("Failed to list jobs: {err}"),
    })?;

    let items: Vec<Value> = rows.iter().map(job_to_json).collect();
    Ok(json!({"ok": true, "mode": "job_list", "count": items.len(), "items": items}))
}

fn job_show(args: JobShowArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    let mut conn = open_db(config)?;
    let job = get_job_by_id(&mut conn, &args.id)?;
    Ok(json!({"ok": true, "mode": "job_show", "job": job_to_json(&job)}))
}

fn job_run_debug(args: JobRunDebugArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    let mut conn = open_db(config)?;
    let job = get_job_by_id(&mut conn, &args.id)?;
    match preflight_job(&job, config) {
        Ok(preflight) => Ok(json!({
            "ok": true,
            "mode": "job_run_debug",
            "job_id": job.id,
            "platform": job.platform,
            "publishable": true,
            "preflight": preflight.details
        })),
        Err(err) => Ok(json!({
            "ok": false,
            "mode": "job_run_debug",
            "job_id": job.id,
            "platform": job.platform,
            "publishable": false,
            "error": err.to_output()
        })),
    }
}

fn worker_run_dry_once(args: WorkerRunArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    if !args.dry_run || !args.once {
        return Err(AppError::Validation {
            message: "Only `publo worker run --dry-run --once` is available currently.".to_string(),
            suggestion: Some(
                "Use both required flags. Live worker publishing is not implemented yet.".to_string(),
            ),
            command: Some("publo worker run --dry-run --once".to_string()),
        });
    }
    if !config.db_path.exists() {
        return Err(AppError::Validation {
            message: format!(
                "Worker database does not exist: {}",
                config.db_path.display()
            ),
            suggestion: Some(
                "Start Publo once or create scheduled jobs before running the dry worker."
                    .to_string(),
            ),
            command: None,
        });
    }

    let now = now_rfc3339_utc();
    let mut conn = open_db(config)?;
    let interrupted = find_expired_worker_claims(&mut conn, &now)?;
    let due_jobs: Vec<JobRow> = sql_query(
        "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
         FROM jobs
         WHERE status = 'scheduled'
           AND run_at_utc IS NOT NULL
           AND run_at_utc <= ?
           AND deleted_at IS NULL
         ORDER BY run_at_utc ASC, created_at ASC",
    )
    .bind::<Text, _>(&now)
    .load(&mut conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to list due scheduled jobs: {err}"),
    })?;

    let mut publishable_count = 0usize;
    let mut blocked_count = 0usize;
    let items: Vec<Value> = due_jobs
        .iter()
        .map(|job| match preflight_job(job, config) {
            Ok(preflight) => {
                publishable_count += 1;
                json!({
                    "job_id": job.id,
                    "platform": job.platform,
                    "file_path": job.file_path,
                    "run_at_utc": job.run_at_utc,
                    "would_publish": true,
                    "preflight": preflight.details
                })
            }
            Err(err) => {
                blocked_count += 1;
                json!({
                    "job_id": job.id,
                    "platform": job.platform,
                    "file_path": job.file_path,
                    "run_at_utc": job.run_at_utc,
                    "would_publish": false,
                    "error": err.to_output()
                })
            }
        })
        .collect();

    Ok(json!({
        "ok": true,
        "mode": "worker_dry_run",
        "once": true,
        "live": false,
        "db_changed": false,
        "now_utc": now,
        "workspace_id": config.workspace_id,
        "interrupted_count": interrupted.len(),
        "interrupted_items": interrupted.iter().map(expired_claim_to_json).collect::<Vec<_>>(),
        "due_count": items.len(),
        "publishable_count": publishable_count,
        "blocked_count": blocked_count,
        "items": items
    }))
}

const WORKER_CLAIM_EXPIRY_MINUTES: i64 = 5;

#[derive(Debug, QueryableByName)]
struct ExpiredWorkerClaimRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Integer)]
    attempt_count: i32,
    #[diesel(sql_type = Nullable<Text>)]
    publish_claim_token: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    publishing_started_at: Option<String>,
}

fn find_expired_worker_claims(
    conn: &mut SqliteConnection,
    now: &str,
) -> Result<Vec<ExpiredWorkerClaimRow>, AppError> {
    let now = chrono::DateTime::parse_from_rfc3339(now).map_err(|err| AppError::Io {
        message: format!("Failed to parse worker clock: {err}"),
    })?;
    let expires_before = (now - chrono::Duration::minutes(WORKER_CLAIM_EXPIRY_MINUTES)).to_rfc3339();
    sql_query(
        "SELECT id, attempt_count, publish_claim_token, publishing_started_at
         FROM jobs
         WHERE status = 'publishing'
           AND publishing_started_at IS NOT NULL
           AND publishing_started_at <= ?
         ORDER BY publishing_started_at ASC",
    )
    .bind::<Text, _>(&expires_before)
    .load(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to find expired worker claims: {err}"),
    })
}

fn expired_claim_to_json(claim: &ExpiredWorkerClaimRow) -> Value {
    json!({
        "job_id": claim.id,
        "attempt_no": claim.attempt_count,
        "publishing_started_at": claim.publishing_started_at,
        "reason": "Publishing outcome unknown: worker claim expired before completion. Verify the platform before rescheduling."
    })
}

#[allow(dead_code)] // Called by the future --live worker before each due-job scan.
fn reconcile_expired_worker_claims(
    conn: &mut SqliteConnection,
    now: &str,
) -> Result<Vec<ExpiredWorkerClaimRow>, AppError> {
    let claims = find_expired_worker_claims(conn, now)?;
    for claim in &claims {
        let Some(token) = claim.publish_claim_token.as_deref() else {
            continue;
        };
        let reason = "Publishing outcome unknown: worker claim expired before completion. Verify the platform before rescheduling.";
        let response_json = json!({
            "ok": false,
            "error_type": "worker_interrupted",
            "message": reason,
            "retryable": false
        })
        .to_string();
        sql_query(
            "UPDATE publish_attempts
             SET finished_at = ?, success = 0, error_type = 'worker_interrupted', error_message = ?,
                 response_json = ?, updated_at = ?, version = version + 1
             WHERE job_id = ? AND attempt_no = ? AND trigger_mode = 'worker' AND claim_token = ? AND finished_at IS NULL",
        )
        .bind::<Text, _>(now)
        .bind::<Text, _>(reason)
        .bind::<Text, _>(&response_json)
        .bind::<Text, _>(now)
        .bind::<Text, _>(&claim.id)
        .bind::<Integer, _>(claim.attempt_count)
        .bind::<Text, _>(token)
        .execute(conn)
        .map_err(|err| AppError::Io {
            message: format!("Failed to close interrupted worker attempt: {err}"),
        })?;
        sql_query(
            "UPDATE jobs
             SET status = 'blocked', status_reason = ?, last_error_type = 'worker_interrupted',
                 last_error_message = ?, last_http_status = NULL,
                 publish_claim_token = NULL, publishing_started_at = NULL,
                 updated_at = ?, version = version + 1, synced_at = NULL, modified_by = 'local'
             WHERE id = ? AND status = 'publishing' AND publish_claim_token = ?",
        )
        .bind::<Text, _>(reason)
        .bind::<Text, _>(reason)
        .bind::<Text, _>(now)
        .bind::<Text, _>(&claim.id)
        .bind::<Text, _>(token)
        .execute(conn)
        .map_err(|err| AppError::Io {
            message: format!("Failed to block interrupted worker job: {err}"),
        })?;
    }
    Ok(claims)
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Wired to the live worker in the next implementation step.
struct WorkerPublishReceipt {
    post_id: Option<String>,
    post_url: Option<String>,
    request_id: Option<String>,
    response: Value,
}

#[allow(dead_code)] // Reused by the live worker after its safe test phase.
struct ClaimedWorkerJob {
    job: JobRow,
    claim_token: String,
}

/// The live worker will implement this with the real platform publishers. Tests use a local fake.
#[allow(dead_code)] // Exercised by worker tests until the live worker is connected.
trait WorkerPublisher {
    fn preflight(&self, job: &JobRow) -> Result<JobPreflightResult, AppError>;
    fn publish(
        &self,
        job: &JobRow,
        preflight: &JobPreflightResult,
    ) -> Result<WorkerPublishReceipt, AppError>;
}

#[allow(dead_code)] // Reused by the live worker after its safe test phase.
fn claim_due_job(
    conn: &mut SqliteConnection,
    job_id: &str,
    now: &str,
) -> Result<Option<ClaimedWorkerJob>, AppError> {
    let claim_token = generate_id();
    conn.immediate_transaction::<Option<JobRow>, diesel::result::Error, _>(|conn| {
        let changed = sql_query(
            "UPDATE jobs
             SET status = 'publishing',
                 attempt_count = attempt_count + 1,
                 publish_claim_token = ?,
                 publishing_started_at = ?,
                 updated_at = ?,
                 version = version + 1,
                 synced_at = NULL,
                 modified_by = 'local'
             WHERE id = ?
               AND status = 'scheduled'
               AND run_at_utc IS NOT NULL
               AND run_at_utc <= ?
               AND deleted_at IS NULL",
        )
        .bind::<Text, _>(&claim_token)
        .bind::<Text, _>(now)
        .bind::<Text, _>(now)
        .bind::<Text, _>(job_id)
        .bind::<Text, _>(now)
        .execute(conn)?;

        if changed == 0 {
            return Ok(None);
        }

        let job = sql_query(
            "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
             FROM jobs WHERE id = ? LIMIT 1",
        )
        .bind::<Text, _>(job_id)
        .get_result::<JobRow>(conn)?;

        sql_query(
            "INSERT INTO publish_attempts (
                id, job_id, attempt_no, platform, workspace_id, owner_user_id, trigger_mode, claim_token,
                started_at, success, created_at, updated_at, modified_by
             ) VALUES (?, ?, ?, ?, ?, ?, 'worker', ?, ?, 0, ?, ?, 'local')",
        )
        .bind::<Text, _>(generate_id())
        .bind::<Text, _>(&job.id)
        .bind::<Integer, _>(job.attempt_count)
        .bind::<Text, _>(job.platform.as_deref().unwrap_or_default())
        .bind::<Text, _>(&job.workspace_id)
        .bind::<Nullable<Text>, _>(job.owner_user_id.as_deref())
        .bind::<Text, _>(&claim_token)
        .bind::<Text, _>(now)
        .bind::<Text, _>(now)
        .bind::<Text, _>(now)
        .execute(conn)?;

        Ok(Some(job))
    })
    .map(|job| job.map(|job| ClaimedWorkerJob { job, claim_token }))
    .map_err(|err| AppError::Io {
        message: format!("Failed to claim due job {job_id}: {err}"),
    })
}

#[allow(dead_code)] // Reused by the live worker after its safe test phase.
fn finish_worker_job_success(
    conn: &mut SqliteConnection,
    job: &JobRow,
    claim_token: &str,
    preflight: &JobPreflightResult,
    receipt: &WorkerPublishReceipt,
    now: &str,
) -> Result<(), AppError> {
    let response_json = serde_json::to_string(&receipt.response).map_err(|err| AppError::Io {
        message: format!("Failed to serialize worker publish response: {err}"),
    })?;
    sql_query(
        "UPDATE publish_attempts
         SET finished_at = ?, success = 1, response_json = ?, post_id = ?, post_url = ?, request_id = ?,
             file_sha256 = ?, text_sha256 = ?, fingerprint = ?, updated_at = ?, version = version + 1
         WHERE job_id = ? AND attempt_no = ? AND trigger_mode = 'worker' AND claim_token = ?",
    )
    .bind::<Text, _>(now)
    .bind::<Text, _>(&response_json)
    .bind::<Nullable<Text>, _>(receipt.post_id.as_deref())
    .bind::<Nullable<Text>, _>(receipt.post_url.as_deref())
    .bind::<Nullable<Text>, _>(receipt.request_id.as_deref())
    .bind::<Text, _>(&preflight.file_sha256)
    .bind::<Text, _>(&preflight.text_sha256)
    .bind::<Text, _>(&preflight.fingerprint)
    .bind::<Text, _>(now)
    .bind::<Text, _>(&job.id)
    .bind::<Integer, _>(job.attempt_count)
    .bind::<Text, _>(claim_token)
    .execute(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to finish successful publish attempt: {err}"),
    })?;

    sql_query(
        "UPDATE jobs
         SET status = 'published', status_reason = NULL, file_sha256 = ?, text_sha256 = ?, fingerprint = ?,
             publish_claim_token = NULL, publishing_started_at = NULL,
             last_error_type = NULL, last_error_message = NULL, last_http_status = NULL,
             updated_at = ?, version = version + 1, synced_at = NULL, modified_by = 'local'
         WHERE id = ? AND status = 'publishing' AND publish_claim_token = ?",
    )
    .bind::<Text, _>(&preflight.file_sha256)
    .bind::<Text, _>(&preflight.text_sha256)
    .bind::<Text, _>(&preflight.fingerprint)
    .bind::<Text, _>(now)
    .bind::<Text, _>(&job.id)
    .bind::<Text, _>(claim_token)
    .execute(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to mark worker job published: {err}"),
    })?;
    Ok(())
}

#[allow(dead_code)] // Reused by the live worker after its safe test phase.
fn finish_worker_job_error(
    conn: &mut SqliteConnection,
    job: &JobRow,
    claim_token: &str,
    status: &str,
    error: &AppError,
    now: &str,
) -> Result<(), AppError> {
    let output = error.to_output();
    let response_json = serde_json::to_string(&output).map_err(|err| AppError::Io {
        message: format!("Failed to serialize worker error response: {err}"),
    })?;
    sql_query(
        "UPDATE publish_attempts
         SET finished_at = ?, success = 0, error_type = ?, error_message = ?, http_status = ?, response_json = ?,
             updated_at = ?, version = version + 1
         WHERE job_id = ? AND attempt_no = ? AND trigger_mode = 'worker' AND claim_token = ?",
    )
    .bind::<Text, _>(now)
    .bind::<Text, _>(output.error_type)
    .bind::<Text, _>(&output.message)
    .bind::<Nullable<Integer>, _>(output.http_status.map(i32::from))
    .bind::<Text, _>(&response_json)
    .bind::<Text, _>(now)
    .bind::<Text, _>(&job.id)
    .bind::<Integer, _>(job.attempt_count)
    .bind::<Text, _>(claim_token)
    .execute(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to finish failed publish attempt: {err}"),
    })?;

    sql_query(
        "UPDATE jobs
         SET status = ?, status_reason = ?, last_error_type = ?, last_error_message = ?, last_http_status = ?,
             publish_claim_token = NULL, publishing_started_at = NULL,
             updated_at = ?, version = version + 1, synced_at = NULL, modified_by = 'local'
         WHERE id = ? AND status = 'publishing' AND publish_claim_token = ?",
    )
    .bind::<Text, _>(status)
    .bind::<Text, _>(&output.message)
    .bind::<Text, _>(output.error_type)
    .bind::<Text, _>(&output.message)
    .bind::<Nullable<Integer>, _>(output.http_status.map(i32::from))
    .bind::<Text, _>(now)
    .bind::<Text, _>(&job.id)
    .bind::<Text, _>(claim_token)
    .execute(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to finalize worker job error: {err}"),
    })?;
    Ok(())
}

#[allow(dead_code)] // Reused by the live worker after its safe test phase.
fn execute_claimed_job(
    conn: &mut SqliteConnection,
    job: &JobRow,
    claim_token: &str,
    publisher: &dyn WorkerPublisher,
    now: &str,
) -> Result<(), AppError> {
    let preflight = match publisher.preflight(job) {
        Ok(preflight) => preflight,
        Err(error) => {
            finish_worker_job_error(conn, job, claim_token, "blocked", &error, now)?;
            return Ok(());
        }
    };
    match publisher.publish(job, &preflight) {
        Ok(receipt) => finish_worker_job_success(conn, job, claim_token, &preflight, &receipt, now),
        Err(error) => finish_worker_job_error(conn, job, claim_token, "failed", &error, now),
    }
}

fn get_job_by_id(conn: &mut SqliteConnection, id: &str) -> Result<JobRow, AppError> {
    let mut rows: Vec<JobRow> = sql_query(
        "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
         FROM jobs
         WHERE id = ?
         LIMIT 1",
    )
    .bind::<Text, _>(id)
    .load(conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to read job by id: {err}"),
    })?;

    rows.pop().ok_or(AppError::Validation {
        message: format!("Job not found: {id}"),
        suggestion: None,
        command: None,
    })
}

struct JobPreflightResult {
    file_sha256: String,
    text_sha256: String,
    fingerprint: String,
    details: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct SchedulePreflightSnapshot {
    pub(crate) file_sha256: String,
    pub(crate) text_sha256: String,
    pub(crate) fingerprint: String,
    pub(crate) details: Value,
}

pub(crate) fn job_preflight_for_file_platform(
    file: PathBuf,
    platform: PlatformArg,
    workspace_id: String,
    owner_user_id: Option<String>,
    by: OperatorArg,
    user_note: Option<String>,
    ai_note: Option<String>,
    ai_model: Option<String>,
    config: &RuntimeConfig,
) -> Result<SchedulePreflightSnapshot, AppError> {
    validate_operator_notes(by, &ai_note, &ai_model)?;
    let file_path = fs::canonicalize(&file).unwrap_or(file.clone());
    let preflight_input = JobRow {
        id: String::new(),
        action_group_id: String::new(),
        content_group_id: String::new(),
        asset_id: String::new(),
        kind: "catalog".to_string(),
        status: "ready".to_string(),
        platform: Some(platform.as_str().to_string()),
        publish_mode: publish_mode_for_platform(platform).map(str::to_string),
        workspace_id,
        owner_user_id,
        operator: Some(by.as_str().to_string()),
        user_note,
        ai_note,
        ai_model,
        tags: "[]".to_string(),
        selected_platforms: "[]".to_string(),
        file_path: file_path.display().to_string(),
        run_at_utc: None,
        timezone: None,
        status_reason: None,
        attempt_count: 0,
        file_sha256: None,
        text_sha256: None,
        fingerprint: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let preflight = preflight_job(&preflight_input, config)?;
    Ok(SchedulePreflightSnapshot {
        file_sha256: preflight.file_sha256,
        text_sha256: preflight.text_sha256,
        fingerprint: preflight.fingerprint,
        details: preflight.details,
    })
}

fn preflight_job(job: &JobRow, config: &RuntimeConfig) -> Result<JobPreflightResult, AppError> {
    let file_path = PathBuf::from(job.file_path.clone());
    let parsed = parse_post_input(&file_path, config)?;
    let media_sha = compute_media_signature(&parsed.media_paths)?;

    match job.platform.as_deref() {
        Some("linkedin") => {
            publish::validate_linkedin_media_count(parsed.media_paths.len())?;
            let auth = load_linkedin_auth()?;
            let signature_text = resolve_signature_text(config, PublishPlatformKind::Linkedin, None)?;
            let commentary_raw = if let Some(sig) = signature_text {
                format!("{}{}", parsed.publish_text, sig)
            } else {
                parsed.publish_text
            };
            let commentary_escaped = escape_little_text_plain(&commentary_raw);
            let text_sha256 = compute_content_sha256(commentary_raw.as_bytes());
            let fingerprint_source = combine_text_and_media_for_fingerprint(&commentary_raw, &media_sha);
            let fingerprint = compute_fingerprint("linkedin", &auth.author_urn, &fingerprint_source);
            Ok(JobPreflightResult {
                file_sha256: parsed.file_sha256,
                text_sha256,
                fingerprint,
                details: json!({
                    "platform": "linkedin",
                    "media_count": parsed.media_paths.len(),
                    "media_paths": parsed.media_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    "text": commentary_raw,
                    "commentary_escaped": commentary_escaped,
                    "author": auth.author_urn
                }),
            })
        }
        Some("x") => {
            if matches!(job.publish_mode.as_deref(), Some("thread")) {
                return Err(AppError::Validation {
                    message: "X thread publish mode is not implemented yet.".to_string(),
                    suggestion: Some("Use X single mode for now.".to_string()),
                    command: None,
                });
            }
            publish::validate_x_media_count(parsed.media_paths.len())?;
            let _auth = load_x_auth()?;
            let signature_text = resolve_signature_text(config, PublishPlatformKind::X, None)?;
            let text = if let Some(sig) = signature_text {
                format!("{}{}", parsed.publish_text, sig)
            } else {
                parsed.publish_text
            };
            if !parsed.media_paths.is_empty() && !x_scope_contains("media.write") {
                return Err(AppError::MissingAuth {
                    message: "X media upload requires media.write scope, but current token scope does not include it.".to_string(),
                    suggestion: Some(
                        "Set X_SCOPES to include media.write, run `publo auth x login`, then retry."
                            .to_string(),
                    ),
                    command: Some("publo auth x login".to_string()),
                });
            }
            publish::validate_x_post_text(&text, false, false)?;
            let text_sha256 = compute_content_sha256(text.as_bytes());
            let author_key = x_author_key();
            let fingerprint_source = combine_text_and_media_for_fingerprint(&text, &media_sha);
            let fingerprint = compute_fingerprint("x", &author_key, &fingerprint_source);
            Ok(JobPreflightResult {
                file_sha256: parsed.file_sha256,
                text_sha256,
                fingerprint,
                details: json!({
                    "platform": "x",
                    "media_count": parsed.media_paths.len(),
                    "media_paths": parsed.media_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    "text": text,
                    "local_preflight": {
                        "weighted_length": publish::x_weighted_length(&text),
                        "cashtag_count": publish::extract_cashtags(&text).len()
                    },
                    "author": author_key
                }),
            })
        }
        Some(other) => Err(AppError::Validation {
            message: format!("Unsupported job platform: {other}"),
            suggestion: Some("Use --platform linkedin|x.".to_string()),
            command: None,
        }),
        None => Err(AppError::Validation {
            message: "Job has no platform yet.".to_string(),
            suggestion: Some("Assign a platform when scheduling.".to_string()),
            command: None,
        }),
    }
}

fn job_to_json(job: &JobRow) -> Value {
    json!({
        "id": job.id,
        "action_group_id": job.action_group_id,
        "content_group_id": job.content_group_id,
        "asset_id": job.asset_id,
        "kind": job.kind,
        "status": job.status,
        "platform": job.platform,
        "publish_mode": job.publish_mode,
        "workspace_id": job.workspace_id,
        "owner_user_id": job.owner_user_id,
        "operator": job.operator,
        "user_note": job.user_note,
        "ai_note": job.ai_note,
        "ai_model": job.ai_model,
        "tags": parse_tags_json(&job.tags),
        "selected_platforms": selected_platforms_from_json(&job.selected_platforms),
        "file_path": job.file_path,
        "run_at_utc": job.run_at_utc,
        "timezone": job.timezone,
        "status_reason": job.status_reason,
        "attempt_count": job.attempt_count,
        "file_sha256": job.file_sha256,
        "text_sha256": job.text_sha256,
        "fingerprint": job.fingerprint,
        "created_at": job.created_at,
        "updated_at": job.updated_at
    })
}

fn publish_mode_for_platform(platform: PlatformArg) -> Option<&'static str> {
    match platform {
        PlatformArg::X => Some("single"),
        PlatformArg::Linkedin => None,
    }
}

fn parse_tags_json(raw: &str) -> Value {
    serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!([]))
}

fn show_linkedin_auth_guide() -> Result<Value, AppError> {
    let client_id = env::var("LINKEDIN_CLIENT_ID").ok();
    let client_secret = env::var("LINKEDIN_CLIENT_SECRET").ok();
    let redirect_uri = env::var("LINKEDIN_REDIRECT_URI").ok();
    let scopes = env::var("LINKEDIN_SCOPES").unwrap_or_else(|_| "w_member_social".to_string());
    let access_token = env::var("LINKEDIN_ACCESS_TOKEN").ok();
    let author_urn = env::var("LINKEDIN_AUTHOR_URN").ok();

    let next = if client_id.is_none() || client_secret.is_none() || redirect_uri.is_none() {
        json!({
            "message": "LinkedIn app settings are incomplete.",
            "required_env": ["LINKEDIN_CLIENT_ID", "LINKEDIN_REDIRECT_URI", "LINKEDIN_CLIENT_SECRET"],
            "command": "publo auth linkedin guide"
        })
    } else if access_token.is_none() {
        json!({
            "message": "No LinkedIn access token found. Start OAuth login.",
            "command": "publo auth linkedin login",
            "scopes": scopes
        })
    } else if author_urn.is_none() {
        json!({
            "message": "Access token exists but LINKEDIN_AUTHOR_URN is missing.",
            "required_env": ["LINKEDIN_AUTHOR_URN"],
            "example": "urn:li:person:xxxxxxxx",
            "command": "publo auth linkedin whoami"
        })
    } else {
        json!({
            "message": "LinkedIn auth appears ready.",
            "command": "publo publish linkedin --file <path> --pass <publish-pass>"
        })
    };

    Ok(json!({
        "ok": true,
        "platform": "linkedin",
        "mode": "auth_guide",
        "next": next
    }))
}

async fn start_x_login(config: &RuntimeConfig) -> Result<Value, AppError> {
    let client_id = env_non_empty("X_CLIENT_ID").ok_or(AppError::MissingAuth {
        message: "X_CLIENT_ID is missing.".to_string(),
        suggestion: Some("Set X_CLIENT_ID in .env from your X app OAuth 2.0 settings.".to_string()),
        command: Some("publo auth x login".to_string()),
    })?;
    let redirect_uri = env_non_empty("X_REDIRECT_URI").ok_or(AppError::MissingAuth {
        message: "X_REDIRECT_URI is missing.".to_string(),
        suggestion: Some("Set X_REDIRECT_URI in .env and match it in your X app callback URL settings.".to_string()),
        command: Some("publo auth x login".to_string()),
    })?;
    let configured_scopes = env_non_empty("X_SCOPES").unwrap_or_else(|| DEFAULT_X_SCOPES.to_string());
    let scopes = ensure_x_required_scopes(&configured_scopes);
    let client_secret = env_non_empty("X_CLIENT_SECRET");

    let redirect = Url::parse(&redirect_uri).map_err(|err| AppError::Validation {
        message: format!("Invalid X_REDIRECT_URI: {err}"),
        suggestion: Some("Use a full URL like http://127.0.0.1:8789/callback".to_string()),
        command: Some("publo auth x login".to_string()),
    })?;
    let host = redirect.host_str().ok_or(AppError::Validation {
        message: "X_REDIRECT_URI must include a host.".to_string(),
        suggestion: Some("Use localhost or 127.0.0.1 callback URL.".to_string()),
        command: Some("publo auth x login".to_string()),
    })?;
    let port = redirect.port().unwrap_or(80);
    let path = redirect.path().to_string();

    let state = generate_state(32);
    let code_verifier = generate_code_verifier(64);
    let code_challenge = pkce_code_challenge(&code_verifier);
    save_x_oauth_state(&XOAuthState {
        state: state.clone(),
        code_verifier: code_verifier.clone(),
        client_id: client_id.clone(),
        redirect_uri: redirect_uri.clone(),
        created_at: Utc::now().to_rfc3339(),
    })?;

    let auth_url = format!(
        "https://twitter.com/i/oauth2/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        encode(&client_id),
        encode(&redirect_uri),
        encode(&scopes),
        encode(&state),
        encode(&code_challenge)
    );

    let bind_target = format!("{host}:{port}");

    let (result_tx, result_rx) = oneshot::channel::<Result<XAuthResult, AppError>>();
    let state_obj = XOAuthCallbackState {
        expected_state: state,
        code_verifier,
        client_id,
        client_secret,
        redirect_uri,
        connect_timeout: config.connect_timeout,
        request_timeout: config.request_timeout,
        result_tx: Arc::new(Mutex::new(Some(result_tx))),
    };

    let listener = tokio::net::TcpListener::bind(bind_target.as_str())
        .await
        .map_err(|err| AppError::Io {
            message: format!("Failed to bind local callback server on {bind_target}: {err}"),
        })?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let app = Router::new()
        .route("/", get(x_oauth_callback_handler))
        .route("/{*rest}", get(x_oauth_callback_handler))
        .with_state(Arc::new(state_obj.clone()));

    let expected_path = if path.is_empty() { "/".to_string() } else { path };
    println!("X auth: local callback server listening on http://{bind_target}{expected_path}");
    println!("X auth: opening browser for authorization...");
    println!("X auth: if browser does not open, use this URL:\n{auth_url}");
    println!("X auth: waiting for callback (Ctrl-C to cancel)...");

    let server_task = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let browser_opened = webbrowser::open(&auth_url).ok().map(|_| true);

    let result = match tokio::time::timeout(Duration::from_secs(300), result_rx).await {
        Ok(Ok(inner)) => inner,
        Ok(Err(_)) => Err(AppError::Io {
            message: "Auth callback channel closed unexpectedly.".to_string(),
        }),
        Err(_) => Err(AppError::Validation {
            message: "Timed out waiting for X OAuth callback.".to_string(),
            suggestion: Some("Retry auth and complete browser consent within 5 minutes.".to_string()),
            command: Some("publo auth x login".to_string()),
        }),
    };

    let _ = shutdown_tx.send(());
    let _ = server_task.await;

    match result {
        Ok(auth_result) => Ok(json!({
            "ok": true,
            "platform": "x",
            "mode": "auth_x_login",
            "browser_opened": browser_opened,
            "access_token_saved": true,
            "refresh_token_saved": auth_result.refresh_token_saved,
            "access_token_expires_in": auth_result.access_token_expires_in,
            "scope": auth_result.scope,
            "token_type": auth_result.token_type,
            "next": {
                "message": "X auth completed. You can publish now.",
                "command": "publo publish x --file <path> --pass <publish-pass>"
            }
        })),
        Err(err) => Err(err),
    }
}

async fn exchange_x_code_manual(
    args: AuthXExchangeArgs,
    config: &RuntimeConfig,
) -> Result<Value, AppError> {
    let state_file = load_x_oauth_state().ok();
    if let Some(s) = &state_file {
        if s.state != args.state {
            return Err(AppError::Validation {
                message: "X OAuth state mismatch.".to_string(),
                suggestion: Some("Use state returned from latest login/auth URL.".to_string()),
                command: Some("publo auth x login".to_string()),
            });
        }
    }

    let client_id = env_non_empty("X_CLIENT_ID")
        .or_else(|| state_file.as_ref().map(|s| s.client_id.clone()))
        .ok_or(AppError::MissingAuth {
            message: "X_CLIENT_ID is missing.".to_string(),
            suggestion: Some("Set X_CLIENT_ID in .env from your X app OAuth 2.0 settings.".to_string()),
            command: Some("publo auth x exchange --code <code> --state <state>".to_string()),
        })?;
    let redirect_uri = env_non_empty("X_REDIRECT_URI")
        .or_else(|| state_file.as_ref().map(|s| s.redirect_uri.clone()))
        .ok_or(AppError::MissingAuth {
            message: "X_REDIRECT_URI is missing.".to_string(),
            suggestion: Some("Set X_REDIRECT_URI in .env to match X app callback URL.".to_string()),
            command: Some("publo auth x exchange --code <code> --state <state>".to_string()),
        })?;
    let code_verifier = args
        .code_verifier
        .or_else(|| state_file.as_ref().map(|s| s.code_verifier.clone()))
        .ok_or(AppError::Validation {
            message: "Missing PKCE code verifier.".to_string(),
            suggestion: Some(
                "Use --code-verifier or run publo auth x login first so verifier is saved."
                    .to_string(),
            ),
            command: Some("publo auth x login".to_string()),
        })?;
    let client_secret = env_non_empty("X_CLIENT_SECRET");

    let auth_result = exchange_x_code_for_token(
        args.code,
        &client_id,
        client_secret.as_deref(),
        &redirect_uri,
        &code_verifier,
        config.connect_timeout,
        config.request_timeout,
    )
    .await?;

    clear_x_oauth_state_file();

    Ok(json!({
        "ok": true,
        "platform": "x",
        "mode": "auth_x_exchange",
        "access_token_saved": true,
        "refresh_token_saved": auth_result.refresh_token_saved,
        "access_token_expires_in": auth_result.access_token_expires_in,
        "scope": auth_result.scope,
        "token_type": auth_result.token_type,
        "next": {
            "message": "X auth exchange completed. You can publish now.",
            "command": "publo publish x --file <path> --pass <publish-pass>"
        }
    }))
}

fn show_x_token_status() -> Result<Value, AppError> {
    Ok(json!({
        "ok": true,
        "platform": "x",
        "mode": "auth_token_status",
        "access_token_present": env_non_empty("X_ACCESS_TOKEN").is_some(),
        "refresh_token_present": env_non_empty("X_REFRESH_TOKEN").is_some(),
        "access_token_expires_in": env_non_empty("X_ACCESS_TOKEN_EXPIRES_IN"),
        "scopes": env_non_empty("X_SCOPES").or_else(|| env_non_empty("X_SCOPE")),
        "token_type": env_non_empty("X_TOKEN_TYPE")
    }))
}

async fn run_x_token_refresh(config: &RuntimeConfig) -> Result<Value, AppError> {
    let refresh_token = env_non_empty("X_REFRESH_TOKEN").ok_or(AppError::MissingAuth {
        message: "No X refresh token found.".to_string(),
        suggestion: Some(
            "Run publo auth x login with offline.access scope, then retry token-refresh."
                .to_string(),
        ),
        command: Some("publo auth x token-status".to_string()),
    })?;
    let client_id = env_non_empty("X_CLIENT_ID").ok_or(AppError::MissingAuth {
        message: "X_CLIENT_ID is missing.".to_string(),
        suggestion: Some("Set X_CLIENT_ID in .env from your X app OAuth 2.0 settings.".to_string()),
        command: Some("publo auth x token-refresh".to_string()),
    })?;
    let client_secret = env_non_empty("X_CLIENT_SECRET");

    let refreshed = refresh_x_access_token(
        &client_id,
        client_secret.as_deref(),
        &refresh_token,
        config.connect_timeout,
        config.request_timeout,
    )
    .await?;

    Ok(json!({
        "ok": true,
        "platform": "x",
        "mode": "auth_token_refresh",
        "token_refreshed": true,
        "access_token_expires_in": refreshed.expires_in,
        "refresh_token_saved": refreshed.refresh_token.is_some(),
        "scope": refreshed.scope,
        "token_type": refreshed.token_type,
        "next": {
            "message": "Token refresh completed.",
            "command": "publo auth x token-status"
        }
    }))
}

async fn x_oauth_callback_handler(
    State(state): State<Arc<XOAuthCallbackState>>,
    Query(query): Query<XCallbackQuery>,
) -> Html<String> {
    let result = process_x_callback(state.clone(), query).await;

    let (title, message, icon, accent): (&str, String, &str, &str) = match &result {
        Ok(_) => (
            "X Authorization Successful",
            "Access token saved successfully. You can close this tab and return to the terminal."
                .to_string(),
            "✅",
            "#138a36",
        ),
        Err(err) => (
            "X Authorization Failed",
            err.to_output().message,
            "😥",
            "#c62828",
        ),
    };

    if let Some(tx) = state.result_tx.lock().await.take() {
        let _ = tx.send(result);
    }

    Html(format!(
        "<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" /><title>{title}</title><style>body{{margin:0;font-family:-apple-system,BlinkMacSystemFont,\"Segoe UI\",Roboto,Helvetica,Arial,sans-serif;background:#f4f6f8;color:#1f2937;display:flex;align-items:center;justify-content:center;min-height:100vh;padding:20px;}} .card{{max-width:560px;width:100%;background:white;border:1px solid #e5e7eb;border-radius:14px;box-shadow:0 10px 30px rgba(0,0,0,.08);padding:28px 24px;text-align:center;}} .icon{{font-size:34px;line-height:1;margin-bottom:8px;}} h2{{margin:0 0 8px;font-size:22px;font-weight:700;color:{accent};}} p{{margin:0;color:#4b5563;font-size:15px;line-height:1.6;}}</style></head><body><div class=\"card\"><div class=\"icon\">{icon}</div><h2>{title}</h2><p>{message}</p></div></body></html>"
    ))
}

async fn process_x_callback(
    state: Arc<XOAuthCallbackState>,
    query: XCallbackQuery,
) -> Result<XAuthResult, AppError> {
    if let Some(error) = query.error {
        return Err(AppError::Validation {
            message: format!(
                "X OAuth returned error: {}{}",
                error,
                query
                    .error_description
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default()
            ),
            suggestion: Some("Retry consent flow and ensure scopes are approved in X app settings.".to_string()),
            command: Some("publo auth x login".to_string()),
        });
    }

    let code = query.code.ok_or(AppError::Validation {
        message: "X callback did not include authorization code.".to_string(),
        suggestion: Some("Retry auth flow and complete consent.".to_string()),
        command: Some("publo auth x login".to_string()),
    })?;
    let returned_state = query.state.ok_or(AppError::Validation {
        message: "X callback did not include state.".to_string(),
        suggestion: Some("Retry auth flow.".to_string()),
        command: Some("publo auth x login".to_string()),
    })?;
    if returned_state != state.expected_state {
        return Err(AppError::Validation {
            message: "X OAuth state mismatch.".to_string(),
            suggestion: Some("Retry auth flow; ensure you use the latest auth URL.".to_string()),
            command: Some("publo auth x login".to_string()),
        });
    }

    let result = exchange_x_code_for_token(
        code,
        &state.client_id,
        state.client_secret.as_deref(),
        &state.redirect_uri,
        &state.code_verifier,
        state.connect_timeout,
        state.request_timeout,
    )
    .await?;
    clear_x_oauth_state_file();
    Ok(result)
}

async fn exchange_x_code_for_token(
    code: String,
    client_id: &str,
    client_secret: Option<&str>,
    redirect_uri: &str,
    code_verifier: &str,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<XAuthResult, AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .map_err(|err| AppError::Http {
            message: format!("Failed to build HTTP client: {err}"),
            status: None,
            api_error: None,
            retryable: false,
        })?;

    let mut params = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect_uri.to_string()),
        ("code_verifier", code_verifier.to_string()),
        ("client_id", client_id.to_string()),
    ];
    if let Some(secret) = client_secret {
        params.push(("client_secret", secret.to_string()));
    }

    let response = client
        .post("https://api.x.com/2/oauth2/token")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("X token exchange request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let maybe_json = response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("X token exchange returned {}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: maybe_json,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }

    let body = response
        .json::<XTokenExchangeResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse X token response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })?;

    upsert_env_value("X_ACCESS_TOKEN", &body.access_token)?;
    if let Some(refresh_token) = body.refresh_token.clone() {
        upsert_env_value("X_REFRESH_TOKEN", &refresh_token)?;
    }
    if let Some(expires_in) = body.expires_in {
        upsert_env_value("X_ACCESS_TOKEN_EXPIRES_IN", &expires_in.to_string())?;
    }
    if let Some(scope) = body.scope.clone() {
        upsert_env_value("X_SCOPES", &scope)?;
    }
    if let Some(token_type) = body.token_type.clone() {
        upsert_env_value("X_TOKEN_TYPE", &token_type)?;
    }

    Ok(XAuthResult {
        access_token_expires_in: body.expires_in,
        refresh_token_saved: body.refresh_token.is_some(),
        scope: body.scope,
        token_type: body.token_type,
    })
}

async fn refresh_x_access_token(
    client_id: &str,
    client_secret: Option<&str>,
    refresh_token: &str,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<XTokenExchangeResponse, AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .map_err(|err| AppError::Http {
            message: format!("Failed to build HTTP client: {err}"),
            status: None,
            api_error: None,
            retryable: false,
        })?;

    let mut params = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
        ("client_id", client_id.to_string()),
    ];
    if let Some(secret) = client_secret {
        params.push(("client_secret", secret.to_string()));
    }

    let response = client
        .post("https://api.x.com/2/oauth2/token")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("X token refresh request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let maybe_json = response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("X token refresh returned {}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: maybe_json,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }

    let body = response
        .json::<XTokenExchangeResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse X token refresh response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })?;

    upsert_env_value("X_ACCESS_TOKEN", &body.access_token)?;
    if let Some(refresh_token) = body.refresh_token.clone() {
        upsert_env_value("X_REFRESH_TOKEN", &refresh_token)?;
    }
    if let Some(expires_in) = body.expires_in {
        upsert_env_value("X_ACCESS_TOKEN_EXPIRES_IN", &expires_in.to_string())?;
    }
    if let Some(scope) = body.scope.clone() {
        upsert_env_value("X_SCOPES", &scope)?;
    }
    if let Some(token_type) = body.token_type.clone() {
        upsert_env_value("X_TOKEN_TYPE", &token_type)?;
    }

    Ok(body)
}

async fn start_linkedin_login(config: &RuntimeConfig) -> Result<Value, AppError> {
    let client_id = env::var("LINKEDIN_CLIENT_ID").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_ID is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_ID in .env from your LinkedIn app settings.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;
    let redirect_uri = env::var("LINKEDIN_REDIRECT_URI").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_REDIRECT_URI is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_REDIRECT_URI in .env and match it in LinkedIn app settings.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;
    let scopes = env::var("LINKEDIN_SCOPES").unwrap_or_else(|_| "w_member_social".to_string());
    let client_secret = env::var("LINKEDIN_CLIENT_SECRET").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_SECRET is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_SECRET in .env from your LinkedIn app settings.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;

    let redirect = Url::parse(&redirect_uri).map_err(|err| AppError::Validation {
        message: format!("Invalid LINKEDIN_REDIRECT_URI: {err}"),
        suggestion: Some("Use a full URL like http://localhost:8788/callback".to_string()),
        command: Some("publo auth linkedin login".to_string()),
    })?;
    let host = redirect.host_str().ok_or(AppError::Validation {
        message: "LINKEDIN_REDIRECT_URI must include a host.".to_string(),
        suggestion: Some("Use localhost or 127.0.0.1 callback URL.".to_string()),
        command: Some("publo auth linkedin login".to_string()),
    })?;
    let port = redirect.port().unwrap_or(80);
    let path = redirect.path().to_string();

    let state = generate_state(32);
    let auth_url = format!(
        "https://www.linkedin.com/oauth/v2/authorization?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
        encode(&client_id),
        encode(&redirect_uri),
        encode(&scopes),
        encode(&state)
    );

    save_oauth_state(&OAuthState {
        state: state.clone(),
        created_at: Utc::now().to_rfc3339(),
    })?;

    let bind_target = format!("{host}:{port}");

    let (result_tx, result_rx) = oneshot::channel::<Result<LinkedinAuthResult, AppError>>();
    let state_obj = LinkedinOAuthCallbackState {
        expected_state: state,
        client_id,
        client_secret,
        redirect_uri,
        connect_timeout: config.connect_timeout,
        request_timeout: config.request_timeout,
        result_tx: Arc::new(Mutex::new(Some(result_tx))),
    };

    let listener = tokio::net::TcpListener::bind(bind_target.as_str())
        .await
        .map_err(|err| AppError::Io {
            message: format!("Failed to bind local callback server on {bind_target}: {err}"),
        })?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let app = Router::new()
        .route("/", get(linkedin_oauth_callback_handler))
        .route("/{*rest}", get(linkedin_oauth_callback_handler))
        .with_state(Arc::new(state_obj.clone()));

    let expected_path = if path.is_empty() { "/".to_string() } else { path };
    println!("LinkedIn auth: local callback server listening on http://{bind_target}{expected_path}");
    println!("LinkedIn auth: opening browser for authorization...");
    println!("LinkedIn auth: if browser does not open, use this URL:\n{auth_url}");
    println!("LinkedIn auth: waiting for callback (Ctrl-C to cancel)...");

    let server_task = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let browser_opened = webbrowser::open(&auth_url).ok().map(|_| true);

    let result = match tokio::time::timeout(Duration::from_secs(300), result_rx).await {
        Ok(Ok(inner)) => inner,
        Ok(Err(_)) => Err(AppError::Io {
            message: "Auth callback channel closed unexpectedly.".to_string(),
        }),
        Err(_) => Err(AppError::Validation {
            message: "Timed out waiting for LinkedIn OAuth callback.".to_string(),
            suggestion: Some("Retry auth and complete browser consent within 5 minutes.".to_string()),
            command: Some("publo auth linkedin login".to_string()),
        }),
    };

    let _ = shutdown_tx.send(());
    let _ = server_task.await;

    match result {
        Ok(auth_result) => Ok(json!({
            "ok": true,
            "platform": "linkedin",
            "mode": "auth_linkedin_login",
            "browser_opened": browser_opened,
            "access_token_saved": true,
            "refresh_token_saved": auth_result.refresh_token_saved,
            "access_token_expires_in": auth_result.access_token_expires_in,
            "author_urn": auth_result.author_urn,
            "author_urn_saved_to_env": true,
            "name": auth_result.profile_name,
            "next": {
                "message": "LinkedIn auth completed. You can publish now.",
                "command": "publo publish linkedin --file <path> --pass <publish-pass>"
            }
        })),
        Err(err) => Err(err),
    }
}

async fn linkedin_oauth_callback_handler(
    State(state): State<Arc<LinkedinOAuthCallbackState>>,
    Query(query): Query<LinkedinCallbackQuery>,
) -> Html<String> {
    let result = process_linkedin_callback(state.clone(), query).await;

    let (title, message, icon, accent): (&str, String, &str, &str) = match &result {
        Ok(_) => (
            "LinkedIn Authorization Successful",
            "Access token and author URN saved successfully. You can close this tab and return to the terminal.".to_string(),
            "✅",
            "#138a36",
        ),
        Err(err) => (
            "LinkedIn Authorization Failed",
            err.to_output().message,
            "😥",
            "#c62828",
        ),
    };

    if let Some(tx) = state.result_tx.lock().await.take() {
        let _ = tx.send(result);
    }

    Html(format!(
        "<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" /><title>{title}</title><style>body{{margin:0;font-family:-apple-system,BlinkMacSystemFont,\"Segoe UI\",Roboto,Helvetica,Arial,sans-serif;background:#f4f6f8;color:#1f2937;display:flex;align-items:center;justify-content:center;min-height:100vh;padding:20px;}} .card{{max-width:560px;width:100%;background:white;border:1px solid #e5e7eb;border-radius:14px;box-shadow:0 10px 30px rgba(0,0,0,.08);padding:28px 24px;text-align:center;}} .icon{{font-size:34px;line-height:1;margin-bottom:8px;}} h2{{margin:0 0 8px;font-size:22px;font-weight:700;color:{accent};}} p{{margin:0;color:#4b5563;font-size:15px;line-height:1.6;}}</style></head><body><div class=\"card\"><div class=\"icon\">{icon}</div><h2>{title}</h2><p>{message}</p></div></body></html>"
    ))
}

async fn process_linkedin_callback(
    state: Arc<LinkedinOAuthCallbackState>,
    query: LinkedinCallbackQuery,
) -> Result<LinkedinAuthResult, AppError> {
    if let Some(error) = query.error {
        return Err(AppError::Validation {
            message: format!(
                "LinkedIn OAuth returned error: {}{}",
                error,
                query
                    .error_description
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default()
            ),
            suggestion: Some("Retry consent flow and ensure scopes are approved in LinkedIn app settings.".to_string()),
            command: Some("publo auth linkedin login".to_string()),
        });
    }

    let code = query.code.ok_or(AppError::Validation {
        message: "LinkedIn callback did not include authorization code.".to_string(),
        suggestion: Some("Retry auth flow and complete consent.".to_string()),
        command: Some("publo auth linkedin login".to_string()),
    })?;
    let returned_state = query.state.ok_or(AppError::Validation {
        message: "LinkedIn callback did not include state.".to_string(),
        suggestion: Some("Retry auth flow.".to_string()),
        command: Some("publo auth linkedin login".to_string()),
    })?;
    if returned_state != state.expected_state {
        return Err(AppError::Validation {
            message: "LinkedIn OAuth state mismatch.".to_string(),
            suggestion: Some("Retry auth flow; ensure you use the latest auth URL.".to_string()),
            command: Some("publo auth linkedin login".to_string()),
        });
    }

    let token = exchange_linkedin_code_for_token(
        code,
        &state.client_id,
        &state.client_secret,
        &state.redirect_uri,
        state.connect_timeout,
        state.request_timeout,
    )
    .await?;

    upsert_env_value("LINKEDIN_ACCESS_TOKEN", &token.access_token)?;
    if let Some(refresh_token) = token.refresh_token.clone() {
        upsert_env_value("LINKEDIN_REFRESH_TOKEN", &refresh_token)?;
    }
    if let Some(expires_in) = token.expires_in {
        upsert_env_value("LINKEDIN_ACCESS_TOKEN_EXPIRES_IN", &expires_in.to_string())?;
    }
    if let Some(refresh_expires_in) = token.refresh_token_expires_in {
        upsert_env_value(
            "LINKEDIN_REFRESH_TOKEN_EXPIRES_IN",
            &refresh_expires_in.to_string(),
        )?;
    }

    let userinfo = fetch_linkedin_userinfo(
        &token.access_token,
        state.connect_timeout,
        state.request_timeout,
    )
    .await?;
    let author_urn = format!("urn:li:person:{}", userinfo.sub);
    upsert_env_value("LINKEDIN_AUTHOR_URN", &author_urn)?;
    clear_oauth_state_file();

    Ok(LinkedinAuthResult {
        access_token_expires_in: token.expires_in,
        refresh_token_saved: token.refresh_token.is_some(),
        author_urn,
        profile_name: userinfo.name,
    })
}

async fn exchange_linkedin_code(
    args: AuthLinkedinExchangeArgs,
    config: &RuntimeConfig,
) -> Result<Value, AppError> {
    let client_id = env::var("LINKEDIN_CLIENT_ID").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_ID is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_ID in .env from your LinkedIn app settings.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;
    let client_secret = env::var("LINKEDIN_CLIENT_SECRET").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_SECRET is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_SECRET in .env from your LinkedIn app settings.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;
    let redirect_uri = env::var("LINKEDIN_REDIRECT_URI").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_REDIRECT_URI is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_REDIRECT_URI in .env and match it in LinkedIn app settings.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;

    validate_oauth_state(&args.state)?;

    let body = exchange_linkedin_code_for_token(
        args.code,
        &client_id,
        &client_secret,
        &redirect_uri,
        config.connect_timeout,
        config.request_timeout,
    )
    .await?;

    upsert_env_value("LINKEDIN_ACCESS_TOKEN", &body.access_token)?;
    if let Some(refresh_token) = body.refresh_token.clone() {
        upsert_env_value("LINKEDIN_REFRESH_TOKEN", &refresh_token)?;
    }
    if let Some(expires_in) = body.expires_in {
        upsert_env_value("LINKEDIN_ACCESS_TOKEN_EXPIRES_IN", &expires_in.to_string())?;
    }
    if let Some(refresh_expires_in) = body.refresh_token_expires_in {
        upsert_env_value(
            "LINKEDIN_REFRESH_TOKEN_EXPIRES_IN",
            &refresh_expires_in.to_string(),
        )?;
    }

    clear_oauth_state_file();

    Ok(json!({
        "ok": true,
        "platform": "linkedin",
        "mode": "auth_exchange",
        "token_saved_to_env": true,
        "access_token_expires_in": body.expires_in,
        "refresh_token_saved": body.refresh_token.is_some(),
        "next": {
            "message": "Resolve and save LINKEDIN_AUTHOR_URN before publishing.",
            "command": "publo auth linkedin whoami"
        }
    }))
}

async fn resolve_linkedin_author_urn(config: &RuntimeConfig) -> Result<Value, AppError> {
    let access_token = env::var("LINKEDIN_ACCESS_TOKEN").map_err(|_| AppError::MissingAuth {
        message: "No LinkedIn access token found.".to_string(),
        suggestion: Some(
            "Run auth flow first: publo auth linkedin login, then exchange.".to_string(),
        ),
        command: Some("publo auth linkedin guide".to_string()),
    })?;

    let userinfo = fetch_linkedin_userinfo(
        &access_token,
        config.connect_timeout,
        config.request_timeout,
    )
    .await?;

    let author_urn = format!("urn:li:person:{}", userinfo.sub);
    upsert_env_value("LINKEDIN_AUTHOR_URN", &author_urn)?;

    Ok(json!({
        "ok": true,
        "platform": "linkedin",
        "mode": "auth_whoami",
        "name": userinfo.name,
        "author_urn": author_urn,
        "author_urn_saved_to_env": true,
        "next": {
            "message": "Author URN is ready. You can publish now.",
            "command": "publo publish linkedin --file <path> --pass <publish-pass>"
        }
    }))
}

async fn fetch_linkedin_userinfo(
    access_token: &str,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<UserInfoResponse, AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .map_err(|err| AppError::Http {
            message: format!("Failed to build HTTP client: {err}"),
            status: None,
            api_error: None,
            retryable: false,
        })?;

    let response = client
        .get("https://api.linkedin.com/v2/userinfo")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("LinkedIn userinfo request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let maybe_json = response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("LinkedIn userinfo returned {}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: maybe_json,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }

    response
        .json::<UserInfoResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse userinfo response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })
}

async fn exchange_linkedin_code_for_token(
    code: String,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<TokenExchangeResponse, AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .map_err(|err| AppError::Http {
            message: format!("Failed to build HTTP client: {err}"),
            status: None,
            api_error: None,
            retryable: false,
        })?;

    let params = [
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect_uri.to_string()),
        ("client_id", client_id.to_string()),
        ("client_secret", client_secret.to_string()),
    ];

    let response = client
        .post("https://www.linkedin.com/oauth/v2/accessToken")
        .form(&params)
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("LinkedIn token exchange request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let maybe_json = response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("LinkedIn token exchange returned {}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: maybe_json,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }

    response
        .json::<TokenExchangeResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse token exchange response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })
}

fn show_linkedin_token_status() -> Result<Value, AppError> {
    let access_token = env_non_empty("LINKEDIN_ACCESS_TOKEN");
    let refresh_token = env_non_empty("LINKEDIN_REFRESH_TOKEN");
    let access_token_expires_in = env_non_empty("LINKEDIN_ACCESS_TOKEN_EXPIRES_IN");
    let refresh_token_expires_in = env_non_empty("LINKEDIN_REFRESH_TOKEN_EXPIRES_IN");
    let author_urn = env_non_empty("LINKEDIN_AUTHOR_URN");

    Ok(json!({
        "ok": true,
        "platform": "linkedin",
        "mode": "auth_token_status",
        "access_token_present": access_token.is_some(),
        "refresh_token_present": refresh_token.is_some(),
        "author_urn_present": author_urn.is_some(),
        "access_token_expires_in": access_token_expires_in,
        "refresh_token_expires_in": refresh_token_expires_in
    }))
}

async fn run_linkedin_token_refresh(config: &RuntimeConfig) -> Result<Value, AppError> {
    let refresh_token = env_non_empty("LINKEDIN_REFRESH_TOKEN").ok_or(AppError::MissingAuth {
        message: "No LinkedIn refresh token found.".to_string(),
        suggestion: Some(
            "Publish already attempts automatic refresh when access token fails. If publish fails, check token-status and retry token-refresh; if that still fails, run login and exchange."
                .to_string(),
        ),
        command: Some("publo auth linkedin token-status".to_string()),
    })?;

    let refreshed = refresh_linkedin_access_token(refresh_token, config).await?;

    Ok(json!({
        "ok": true,
        "platform": "linkedin",
        "mode": "auth_token_refresh",
        "token_refreshed": true,
        "access_token_expires_in": refreshed.expires_in,
        "refresh_token_saved": refreshed.refresh_token.is_some(),
        "next": {
            "message": "Token refresh completed.",
            "command": "publo auth linkedin token-status"
        }
    }))
}

async fn publish_linkedin_cli(
    args: PublishLinkedinArgs,
    config: &RuntimeConfig,
) -> Result<Value, AppError> {
    if !args.debug {
        validate_publish_pass(args.pass.as_deref(), config)?;
    }
    publish_linkedin(args, config).await
}

async fn publish_linkedin(
    args: PublishLinkedinArgs,
    config: &RuntimeConfig,
) -> Result<Value, AppError> {
    if !args.file.exists() {
        return Err(AppError::Validation {
            message: format!("Content file does not exist: {}", args.file.display()),
            suggestion: Some("Check the file path and run the same command again.".to_string()),
            command: None,
        });
    }

    let parsed = parse_post_input(&args.file, config)?;
    publish::validate_linkedin_media_count(parsed.media_paths.len())?;
    let mut commentary_raw = parsed.publish_text;
    let signature_override = signature_cli_override(args.add_signature, args.no_signature);
    let mut signature_applied = false;
    if let Some(signature) =
        resolve_signature_text(config, PublishPlatformKind::Linkedin, signature_override)?
    {
        commentary_raw.push_str(&signature);
        signature_applied = true;
    }
    let commentary = escape_little_text_plain(&commentary_raw);

    let auth = load_linkedin_auth()?;
    let media_sha = compute_media_signature(&parsed.media_paths)?;
    let fingerprint_source = combine_text_and_media_for_fingerprint(&commentary_raw, &media_sha);
    let file_sha256 = parsed.file_sha256;
    let text_sha256 = compute_content_sha256(commentary_raw.as_bytes());
    let fingerprint = compute_fingerprint("linkedin", &auth.author_urn, &fingerprint_source);
    let mut payload = json!({
        "author": auth.author_urn,
        "commentary": commentary,
        "visibility": "PUBLIC",
        "distribution": {
            "feedDistribution": "MAIN_FEED",
            "targetEntities": [],
            "thirdPartyDistributionChannels": []
        },
        "lifecycleState": "PUBLISHED",
        "isReshareDisabledByAuthor": false
    });

    maybe_block_duplicate(
        "linkedin",
        &auth.author_urn,
        &fingerprint,
        &file_sha256,
        args.allow_duplicate,
    )?;

    if args.debug {
        let payload_preview = if parsed.media_paths.is_empty() {
            payload.clone()
        } else if parsed.media_paths.len() == 1 {
            let mut preview = payload.clone();
            preview
                .as_object_mut()
                .expect("payload object")
                .insert(
                    "content".to_string(),
                    json!({
                        "media": {
                            "id": "<resolved-via-linkedin-upload>"
                        }
                    }),
                );
            preview
        } else {
            let mut preview = payload.clone();
            let images_preview: Vec<Value> = parsed
                .media_paths
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    json!({
                        "id": format!("<resolved-via-linkedin-upload-{}>", i + 1)
                    })
                })
                .collect();
            preview
                .as_object_mut()
                .expect("payload object")
                .insert(
                    "content".to_string(),
                    json!({
                        "multiImage": {
                            "images": images_preview
                        }
                    }),
                );
            preview
        };

        return Ok(json!({
            "ok": true,
            "platform": "linkedin",
            "mode": "debug",
            "would_publish": true,
            "signature_applied": signature_applied,
            "media_count": parsed.media_paths.len(),
            "media_paths": parsed.media_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "commentary_raw": commentary_raw,
            "commentary_escaped": commentary,
            "payload": payload_preview,
            "fingerprint": fingerprint,
            "file_sha256": file_sha256,
            "text_sha256": text_sha256
        }));
    }

    let request_timeout = args
        .timeout_seconds
        .map(Duration::from_secs)
        .unwrap_or(config.request_timeout);

    let client = reqwest::Client::builder()
        .connect_timeout(config.connect_timeout)
        .timeout(request_timeout)
        .build()
        .map_err(|err| AppError::Http {
            message: format!("Failed to build HTTP client: {err}"),
            status: None,
            api_error: None,
            retryable: false,
        })?;

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", auth.access_token)).map_err(|err| {
            AppError::Validation {
                message: format!("Invalid access token format: {err}"),
                suggestion: Some("Re-authenticate and store a valid token.".to_string()),
                command: Some("publo auth linkedin guide".to_string()),
            }
        })?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "LinkedIn-Version",
        HeaderValue::from_str(&auth.version).map_err(|err| AppError::Validation {
            message: format!("Invalid LINKEDIN_API_VERSION value: {err}"),
            suggestion: Some("Use a YYYYMM value like 202601.".to_string()),
            command: None,
        })?,
    );
    headers.insert(
        "X-Restli-Protocol-Version",
        HeaderValue::from_static("2.0.0"),
    );

    let response = client
        .post("https://api.linkedin.com/rest/posts");

    let mut media_urns: Vec<String> = Vec::new();
    if !parsed.media_paths.is_empty() {
        for path in &parsed.media_paths {
            let image_urn = linkedin_upload_image(
                &client,
                &auth.access_token,
                &auth.version,
                &auth.author_urn,
                path,
            )
            .await?;
            media_urns.push(image_urn);
        }
        let content_value = if media_urns.len() == 1 {
            json!({
                "media": {
                    "id": media_urns[0]
                }
            })
        } else {
            let images: Vec<Value> = media_urns
                .iter()
                .map(|id| {
                    json!({
                        "id": id
                    })
                })
                .collect();
            json!({
                "multiImage": {
                    "images": images
                }
            })
        };
        payload
            .as_object_mut()
            .expect("payload object")
            .insert("content".to_string(), content_value);
    }

    let mut response = response
        .headers(headers.clone())
        .json(&payload)
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("LinkedIn request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let mut token_refreshed = false;
    if should_try_refresh(response.status()) {
        if let Some(refresh_token) = auth.refresh_token.clone() {
            let refreshed = refresh_linkedin_access_token(refresh_token, config).await?;
            token_refreshed = true;

            let mut retry_headers = headers.clone();
            retry_headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", refreshed.access_token)).map_err(|err| {
                    AppError::Validation {
                        message: format!("Invalid refreshed access token format: {err}"),
                        suggestion: Some("Re-run auth flow and retry publish.".to_string()),
                        command: Some("publo auth linkedin login".to_string()),
                    }
                })?,
            );

            response = client
                .post("https://api.linkedin.com/rest/posts")
                .headers(retry_headers)
                .json(&payload)
                .send()
                .await
                .map_err(|err| AppError::Http {
                    message: format!("LinkedIn retry request failed after refresh: {err}"),
                    status: None,
                    api_error: None,
                    retryable: err.is_timeout() || err.is_connect(),
                })?;
        }
    }

    let status = response.status();
    let request_id = response
        .headers()
        .get("x-restli-id")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());

    if !status.is_success() {
        let maybe_json = response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("LinkedIn API returned {}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: maybe_json,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }

    let body = response.json::<Value>().await.ok();
    let post_id = body
        .as_ref()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .or(request_id.clone());
    let post_url = post_id
        .as_ref()
        .map(|id| format!("https://www.linkedin.com/feed/update/{id}/"));

    let published_at = Utc::now().to_rfc3339();
    let output = SuccessOutput {
        ok: true,
        platform: "linkedin",
        post_id: post_id.clone(),
        post_url: post_url.clone(),
        request_id: request_id.clone(),
        published_at: published_at.clone(),
    };

    let mut value = serde_json::to_value(output).map_err(|err| AppError::Io {
        message: format!("Failed to serialize success output: {err}"),
    })?;

    if let Some(obj) = value.as_object_mut() {
        obj.insert("token_refreshed".to_string(), json!(token_refreshed));
        obj.insert("signature_applied".to_string(), json!(signature_applied));
        obj.insert("media_count".to_string(), json!(parsed.media_paths.len()));
        obj.insert("media_urns".to_string(), json!(media_urns));
    }
    attach_publish_metadata(&mut value, &fingerprint, &file_sha256, &text_sha256, true);

    let log_entry = PublishLogEntry {
        platform: "linkedin".to_string(),
        author_urn: auth.author_urn,
        file_path: args.file.display().to_string(),
        fingerprint,
        file_sha256,
        text_sha256,
        post_id,
        post_url,
        request_id,
        published_at,
    };
    append_publish_log(&log_entry)?;

    Ok(value)
}

async fn publish_x_cli(args: PublishXArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    if !args.debug {
        validate_publish_pass(args.pass.as_deref(), config)?;
    }
    publish_x(args, config).await
}

async fn publish_x(args: PublishXArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    if !args.file.exists() {
        return Err(AppError::Validation {
            message: format!("Content file does not exist: {}", args.file.display()),
            suggestion: Some("Check the file path and run the same command again.".to_string()),
            command: None,
        });
    }

    let parsed = parse_post_input(&args.file, config)?;
    publish::validate_x_media_count(parsed.media_paths.len())?;
    let mut text = parsed.publish_text;
    let signature_override = signature_cli_override(args.add_signature, args.no_signature);
    let mut signature_applied = false;
    if let Some(signature) =
        resolve_signature_text(config, PublishPlatformKind::X, signature_override)?
    {
        text.push_str(&signature);
        signature_applied = true;
    }
    if !parsed.media_paths.is_empty() && !x_scope_contains("media.write") {
        return Err(AppError::MissingAuth {
            message: "X media upload requires media.write scope, but current token scope does not include it.".to_string(),
            suggestion: Some(
                "Set X_SCOPES to include media.write, run `publo auth x login`, then retry publish."
                    .to_string(),
            ),
            command: Some("publo auth x login".to_string()),
        });
    }
    let bypass_duplicate = args.force || args.allow_duplicate;
    let bypass_cashtag = args.force || args.allow_cashtag;
    let bypass_length = args.force || args.allow_length;
    let cashtag_count = publish::extract_cashtags(&text).len();
    let weighted_len = publish::x_weighted_length(&text);

    publish::validate_x_post_text(&text, bypass_cashtag, bypass_length)?;

    let auth = load_x_auth()?;
    let author_key = x_author_key();
    let media_sha = compute_media_signature(&parsed.media_paths)?;
    let fingerprint_source = combine_text_and_media_for_fingerprint(&text, &media_sha);
    let file_sha256 = parsed.file_sha256;
    let text_sha256 = compute_content_sha256(text.as_bytes());
    let fingerprint = compute_fingerprint("x", &author_key, &fingerprint_source);
    maybe_block_duplicate("x", &author_key, &fingerprint, &file_sha256, bypass_duplicate)?;

    if args.debug {
        let payload_preview = if parsed.media_paths.is_empty() {
            json!({ "text": text })
        } else {
            json!({
                "text": text,
                "media": {
                    "media_ids": parsed
                        .media_paths
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("<resolved-via-x-upload-{}>", i + 1))
                        .collect::<Vec<_>>()
                }
            })
        };
        return Ok(json!({
            "ok": true,
            "platform": "x",
            "mode": "debug",
            "would_publish": true,
            "signature_applied": signature_applied,
            "media_count": parsed.media_paths.len(),
            "media_paths": parsed.media_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "text": text,
            "payload": payload_preview,
            "fingerprint": fingerprint,
            "file_sha256": file_sha256,
            "text_sha256": text_sha256,
            "duplicate_guard": if bypass_duplicate { "bypassed" } else { "checked" },
            "local_preflight": {
                "weighted_length": weighted_len,
                "cashtag_count": cashtag_count
            },
            "auth_present": !auth.access_token.trim().is_empty()
        }));
    }

    let request_timeout = args
        .timeout_seconds
        .map(Duration::from_secs)
        .unwrap_or(config.request_timeout);

    let client = reqwest::Client::builder()
        .connect_timeout(config.connect_timeout)
        .timeout(request_timeout)
        .build()
        .map_err(|err| AppError::Http {
            message: format!("Failed to build HTTP client: {err}"),
            status: None,
            api_error: None,
            retryable: false,
        })?;

    let mut active_access_token = auth.access_token.clone();
    let mut token_refreshed = false;
    let mut media_ids: Vec<String> = Vec::new();

    let (body, request_id, final_media_ids): (Option<Value>, Option<String>, Vec<String>) = loop {
        media_ids.clear();
        let mut upload_auth_failed = false;
        for path in &parsed.media_paths {
            let (media_id, upload_status, _upload_error) =
                x_upload_media_image(&client, &active_access_token, path).await?;
            if upload_status == Some(401) {
                upload_auth_failed = true;
                break;
            }
            media_ids.push(media_id);
        }

        if upload_auth_failed {
            if token_refreshed {
                return Err(AppError::MissingAuth {
                    message: "X access token is invalid or expired for media upload.".to_string(),
                    suggestion: Some("Run publo auth x login and retry publish.".to_string()),
                    command: Some("publo auth x login".to_string()),
                });
            }
            let refresh_token = auth.refresh_token.clone().ok_or(AppError::MissingAuth {
                message: "X access token is invalid or expired for media upload.".to_string(),
                suggestion: Some("Run publo auth x login and retry publish.".to_string()),
                command: Some("publo auth x login".to_string()),
            })?;
            let client_id = env_non_empty("X_CLIENT_ID").ok_or(AppError::MissingAuth {
                message: "X_CLIENT_ID is missing.".to_string(),
                suggestion: Some("Set X_CLIENT_ID in .env from your X app OAuth 2.0 settings.".to_string()),
                command: Some("publo auth x token-refresh".to_string()),
            })?;
            let client_secret = env_non_empty("X_CLIENT_SECRET");
            let refreshed = refresh_x_access_token(
                &client_id,
                client_secret.as_deref(),
                &refresh_token,
                config.connect_timeout,
                config.request_timeout,
            )
            .await?;
            active_access_token = refreshed.access_token;
            token_refreshed = true;
            continue;
        }

        let payload = if media_ids.is_empty() {
            json!({ "text": text })
        } else {
            json!({
                "text": text,
                "media": {
                    "media_ids": media_ids
                }
            })
        };

        let response = client
            .post("https://api.x.com/2/tweets")
            .header(AUTHORIZATION, format!("Bearer {}", active_access_token))
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|err| AppError::Http {
                message: format!("X request failed: {err}"),
                status: None,
                api_error: None,
                retryable: err.is_timeout() || err.is_connect(),
            })?;

        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());

        if status.as_u16() == 401 {
            if token_refreshed {
                return Err(AppError::MissingAuth {
                    message: "X access token is invalid or expired.".to_string(),
                    suggestion: Some("Run publo auth x login and retry publish.".to_string()),
                    command: Some("publo auth x login".to_string()),
                });
            }
            let refresh_token = auth.refresh_token.clone().ok_or(AppError::MissingAuth {
                message: "X access token is invalid or expired.".to_string(),
                suggestion: Some("Run publo auth x login and retry publish.".to_string()),
                command: Some("publo auth x login".to_string()),
            })?;
            let client_id = env_non_empty("X_CLIENT_ID").ok_or(AppError::MissingAuth {
                message: "X_CLIENT_ID is missing.".to_string(),
                suggestion: Some("Set X_CLIENT_ID in .env from your X app OAuth 2.0 settings.".to_string()),
                command: Some("publo auth x token-refresh".to_string()),
            })?;
            let client_secret = env_non_empty("X_CLIENT_SECRET");
            let refreshed = refresh_x_access_token(
                &client_id,
                client_secret.as_deref(),
                &refresh_token,
                config.connect_timeout,
                config.request_timeout,
            )
            .await?;
            active_access_token = refreshed.access_token;
            token_refreshed = true;
            continue;
        }

        if !status.is_success() {
            let mut maybe_json = response.json::<Value>().await.ok();
            if status.as_u16() == 403 && is_generic_x_forbidden(maybe_json.as_ref()) {
                if weighted_len > 280 {
                    let mut err = maybe_json.unwrap_or_else(|| json!({}));
                    if !err.is_object() {
                        err = json!({ "provider_error": err });
                    }
                    if let Some(obj) = err.as_object_mut() {
                        obj.insert("local_hint".to_string(), json!("x_likely_over_length"));
                        obj.insert("local_weighted_length".to_string(), json!(weighted_len));
                        obj.insert("local_weighted_limit".to_string(), json!(280));
                    }
                    maybe_json = Some(err);
                } else if cashtag_count > 1 {
                    let mut err = maybe_json.unwrap_or_else(|| json!({}));
                    if !err.is_object() {
                        err = json!({ "provider_error": err });
                    }
                    if let Some(obj) = err.as_object_mut() {
                        obj.insert("local_hint".to_string(), json!("x_likely_cashtag_limit"));
                        obj.insert("local_cashtag_count".to_string(), json!(cashtag_count));
                        obj.insert("local_cashtag_limit".to_string(), json!(1));
                    }
                    maybe_json = Some(err);
                }
            }
            return Err(AppError::Http {
                message: format!("X API returned {}", status.as_u16()),
                status: Some(status.as_u16()),
                api_error: maybe_json,
                retryable: status.is_server_error() || status.as_u16() == 429,
            });
        }

        let body = response.json::<Value>().await.ok();
        break (body, request_id, media_ids.clone());
    };

    let post_id = body
        .as_ref()
        .and_then(|v| v.get("data"))
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let post_url = post_id
        .as_ref()
        .map(|id| format!("https://x.com/i/web/status/{id}"));

    let output = SuccessOutput {
        ok: true,
        platform: "x",
        post_id: post_id.clone(),
        post_url: post_url.clone(),
        request_id: request_id.clone(),
        published_at: Utc::now().to_rfc3339(),
    };
    let mut value = serde_json::to_value(output).map_err(|err| AppError::Io {
        message: format!("Failed to serialize success output: {err}"),
    })?;
    attach_publish_metadata(
        &mut value,
        &fingerprint,
        &file_sha256,
        &text_sha256,
        !bypass_duplicate,
    );
    if let Some(obj) = value.as_object_mut() {
        obj.insert("signature_applied".to_string(), json!(signature_applied));
        obj.insert("token_refreshed".to_string(), json!(token_refreshed));
        obj.insert("media_count".to_string(), json!(parsed.media_paths.len()));
        obj.insert("media_paths".to_string(), json!(
            parsed.media_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
        ));
        obj.insert("media_ids".to_string(), json!(final_media_ids));
    }

    let published_at = value
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let log_entry = PublishLogEntry {
        platform: "x".to_string(),
        author_urn: author_key,
        file_path: args.file.display().to_string(),
        fingerprint,
        file_sha256,
        text_sha256,
        post_id,
        post_url,
        request_id,
        published_at,
    };
    append_publish_log(&log_entry)?;

    Ok(value)
}

fn is_generic_x_forbidden(api_error: Option<&Value>) -> bool {
    let Some(err) = api_error else {
        return false;
    };
    let detail = err
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let title = err
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let typ = err
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    detail == "you are not permitted to perform this action."
        && title == "forbidden"
        && typ == "about:blank"
}

fn parse_post_input(file_path: &PathBuf, config: &RuntimeConfig) -> Result<ParsedPostInput, AppError> {
    let content = fs::read_to_string(file_path).map_err(|err| AppError::Io {
        message: format!("Failed to read content file: {err}"),
    })?;

    let media_refs = publish::extract_obsidian_embeds(&content);
    let media_paths = publish::resolve_media_paths(file_path, &media_refs, &config.media_lookup_paths)?;

    let publish_text = publish::extract_publish_text(&content);
    if publish_text.is_empty() {
        return Err(AppError::Validation {
            message: "Post text is empty after removing metadata and image placeholders.".to_string(),
            suggestion: Some("Add publishable text after the separator (---).".to_string()),
            command: None,
        });
    }

    Ok(ParsedPostInput {
        publish_text,
        media_paths,
        file_sha256: compute_content_sha256(content.as_bytes()),
    })
}

fn compute_media_signature(media_paths: &[PathBuf]) -> Result<String, AppError> {
    if media_paths.is_empty() {
        return Ok(String::new());
    }
    let mut parts: Vec<String> = Vec::new();
    for path in media_paths {
        let bytes = fs::read(path).map_err(|err| AppError::Io {
            message: format!("Failed to read media file '{}': {err}", path.display()),
        })?;
        parts.push(compute_content_sha256(&bytes));
    }
    Ok(parts.join(","))
}

fn combine_text_and_media_for_fingerprint(text: &str, media_signature: &str) -> String {
    if media_signature.is_empty() {
        return text.to_string();
    }
    format!("{text}\n[media_sha256:{media_signature}]")
}

#[derive(Copy, Clone)]
enum PublishPlatformKind {
    Linkedin,
    X,
}

fn signature_cli_override(add_signature: bool, no_signature: bool) -> Option<bool> {
    if add_signature {
        Some(true)
    } else if no_signature {
        Some(false)
    } else {
        None
    }
}

fn resolve_signature_text(
    config: &RuntimeConfig,
    platform: PublishPlatformKind,
    cli_override_enabled: Option<bool>,
) -> Result<Option<String>, AppError> {
    let platform_layer = match platform {
        PublishPlatformKind::Linkedin => &config.linkedin_signature,
        PublishPlatformKind::X => &config.x_signature,
    };
    let platform_name = match platform {
        PublishPlatformKind::Linkedin => "linkedin",
        PublishPlatformKind::X => "x",
    };

    let enabled = cli_override_enabled
        .or(platform_layer.enabled)
        .or(config.global_signature.enabled)
        .unwrap_or(false);

    if !enabled {
        return Ok(None);
    }

    let text = platform_layer
        .text
        .as_ref()
        .or(config.global_signature.text.as_ref())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or(AppError::Validation {
            message: format!(
                "Signature is enabled for {} but no signature text is configured.",
                platform_name
            ),
            suggestion: Some(
                "Set [signature].text (or [platform.<name>.signature].text) in config.toml."
                    .to_string(),
            ),
            command: None,
        })?;

    Ok(Some(text))
}

fn x_author_key() -> String {
    env_non_empty("X_AUTHOR_ID")
        .or_else(|| env_non_empty("X_AUTHOR_HANDLE"))
        .unwrap_or_else(|| "x:self".to_string())
}

fn ensure_x_required_scopes(input: &str) -> String {
    let mut parts: Vec<String> = input
        .split_whitespace()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .collect();

    for required in ["tweet.read", "tweet.write", "users.read", "media.write"] {
        if !parts.iter().any(|p| p == required) {
            parts.push(required.to_string());
        }
    }

    if !parts.iter().any(|p| p == "offline.access") {
        parts.push("offline.access".to_string());
    }

    parts.join(" ")
}

fn x_scope_contains(scope: &str) -> bool {
    env_non_empty("X_SCOPES")
        .map(|s| s.split_whitespace().any(|p| p == scope))
        .unwrap_or(false)
}

fn maybe_block_duplicate(
    platform: &str,
    author_key: &str,
    fingerprint: &str,
    file_sha256: &str,
    allow_duplicate: bool,
) -> Result<(), AppError> {
    if allow_duplicate {
        return Ok(());
    }
    if let Some(existing) = find_existing_publish(platform, author_key, fingerprint)? {
        return Err(AppError::DuplicatePublish {
            message: "Duplicate publish blocked for same platform/author/content.".to_string(),
            existing_post_id: existing.post_id,
            existing_post_url: existing.post_url,
            file_sha256: file_sha256.to_string(),
            fingerprint: fingerprint.to_string(),
            existing_published_at: existing.published_at,
        });
    }
    Ok(())
}

fn attach_publish_metadata(
    value: &mut Value,
    fingerprint: &str,
    file_sha256: &str,
    text_sha256: &str,
    duplicate_checked: bool,
) {
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "duplicate_guard".to_string(),
            json!(if duplicate_checked { "checked" } else { "bypassed" }),
        );
        obj.insert("fingerprint".to_string(), json!(fingerprint));
        obj.insert("file_sha256".to_string(), json!(file_sha256));
        obj.insert("text_sha256".to_string(), json!(text_sha256));
    }
}

fn compute_fingerprint(platform: &str, author_urn: &str, commentary: &str) -> String {
    let normalized = commentary.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut hasher = Sha256::new();
    hasher.update(platform.as_bytes());
    hasher.update(b"|");
    hasher.update(author_urn.as_bytes());
    hasher.update(b"|");
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn escape_little_text_plain(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '|' | '{' | '}' | '@' | '[' | ']' | '(' | ')' | '<' | '>' | '#' | '\\' | '*' | '_'
            | '~' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn compute_content_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn find_existing_publish(
    platform: &str,
    author_urn: &str,
    fingerprint: &str,
) -> Result<Option<PublishLogEntry>, AppError> {
    let publish_log_path = config::resolve_runtime_paths().publish_log_path;
    let Ok(raw) = fs::read_to_string(&publish_log_path) else {
        return Ok(None);
    };
    for line in raw.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: PublishLogEntry = serde_json::from_str(line).map_err(|err| AppError::Io {
            message: format!("Failed to parse publish log entry: {err}"),
        })?;
        if entry.platform == platform
            && entry.author_urn == author_urn
            && entry.fingerprint == fingerprint
        {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

fn append_publish_log(entry: &PublishLogEntry) -> Result<(), AppError> {
    let paths = config::resolve_runtime_paths();
    if let Some(parent) = paths.publish_log_path.parent() {
        fs::create_dir_all(parent).map_err(|err| AppError::Io {
            message: format!(
                "Failed to create publish log directory '{}': {err}",
                parent.display()
            ),
        })?;
    }
    let line = serde_json::to_string(entry).map_err(|err| AppError::Io {
        message: format!("Failed to encode publish log entry: {err}"),
    })?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.publish_log_path)
        .map_err(|err| AppError::Io {
            message: format!("Failed to open publish log: {err}"),
        })?;
    writeln!(file, "{line}").map_err(|err| AppError::Io {
        message: format!("Failed to append publish log: {err}"),
    })
}

fn should_try_refresh(status: reqwest::StatusCode) -> bool {
    status.as_u16() == 401
}

async fn refresh_linkedin_access_token(
    refresh_token: String,
    config: &RuntimeConfig,
) -> Result<TokenExchangeResponse, AppError> {
    let client_id = env::var("LINKEDIN_CLIENT_ID").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_ID is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_ID in .env from your LinkedIn app settings.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;
    let client_secret = env::var("LINKEDIN_CLIENT_SECRET").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_SECRET is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_SECRET in .env from your LinkedIn app settings.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;

    let client = reqwest::Client::builder()
        .connect_timeout(config.connect_timeout)
        .timeout(config.request_timeout)
        .build()
        .map_err(|err| AppError::Http {
            message: format!("Failed to build HTTP client: {err}"),
            status: None,
            api_error: None,
            retryable: false,
        })?;

    let params = [
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let response = client
        .post("https://www.linkedin.com/oauth/v2/accessToken")
        .form(&params)
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("LinkedIn token refresh request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let maybe_json = response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("LinkedIn token refresh returned {}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: maybe_json,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }

    let body = response
        .json::<TokenExchangeResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse token refresh response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })?;

    upsert_env_value("LINKEDIN_ACCESS_TOKEN", &body.access_token)?;
    if let Some(expires_in) = body.expires_in {
        upsert_env_value("LINKEDIN_ACCESS_TOKEN_EXPIRES_IN", &expires_in.to_string())?;
    }
    if let Some(new_refresh_token) = body.refresh_token.clone() {
        upsert_env_value("LINKEDIN_REFRESH_TOKEN", &new_refresh_token)?;
    }
    if let Some(refresh_expires_in) = body.refresh_token_expires_in {
        upsert_env_value(
            "LINKEDIN_REFRESH_TOKEN_EXPIRES_IN",
            &refresh_expires_in.to_string(),
        )?;
    }

    Ok(body)
}

async fn linkedin_upload_image(
    client: &reqwest::Client,
    access_token: &str,
    linkedin_version: &str,
    owner_urn: &str,
    image_path: &PathBuf,
) -> Result<String, AppError> {
    let init_response = client
        .post("https://api.linkedin.com/rest/images?action=initializeUpload")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(CONTENT_TYPE, "application/json")
        .header("LinkedIn-Version", linkedin_version)
        .header("X-Restli-Protocol-Version", "2.0.0")
        .json(&json!({
            "initializeUploadRequest": {
                "owner": owner_urn
            }
        }))
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("LinkedIn image initializeUpload request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let init_status = init_response.status();
    if !init_status.is_success() {
        let maybe_json = init_response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("LinkedIn image initializeUpload returned {}", init_status.as_u16()),
            status: Some(init_status.as_u16()),
            api_error: maybe_json,
            retryable: init_status.is_server_error() || init_status.as_u16() == 429,
        });
    }

    let init_body = init_response
        .json::<LinkedinImageInitResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse LinkedIn image initializeUpload response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })?;

    let image_bytes = fs::read(image_path).map_err(|err| AppError::Io {
        message: format!("Failed to read image '{}': {err}", image_path.display()),
    })?;
    let content_type = media_content_type(image_path)?;

    let upload_response = client
        .put(&init_body.value.upload_url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(CONTENT_TYPE, content_type)
        .body(image_bytes)
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("LinkedIn image upload request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let upload_status = upload_response.status();
    if !upload_status.is_success() {
        let maybe_json = upload_response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("LinkedIn image upload returned {}", upload_status.as_u16()),
            status: Some(upload_status.as_u16()),
            api_error: maybe_json,
            retryable: upload_status.is_server_error() || upload_status.as_u16() == 429,
        });
    }

    Ok(init_body.value.image)
}

async fn x_upload_media_image(
    client: &reqwest::Client,
    access_token: &str,
    image_path: &PathBuf,
) -> Result<(String, Option<u16>, Option<Value>), AppError> {
    let image_bytes = fs::read(image_path).map_err(|err| AppError::Io {
        message: format!("Failed to read image '{}': {err}", image_path.display()),
    })?;
    let media_b64 = STANDARD.encode(image_bytes);
    let media_type = media_content_type(image_path)?;

    let response = client
        .post("https://api.x.com/2/media/upload")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(CONTENT_TYPE, "application/json")
        .json(&json!({
            "media": media_b64,
            "media_category": "tweet_image",
            "media_type": media_type
        }))
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("X media upload request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

    let status = response.status();
    if status.as_u16() == 401 {
        return Ok((String::new(), Some(401), response.json::<Value>().await.ok()));
    }
    if status.as_u16() == 403 {
        return Err(AppError::MissingAuth {
            message: "X media upload forbidden. Token may be missing media.write scope or app access for media upload.".to_string(),
            suggestion: Some(
                "Ensure X_SCOPES includes media.write, re-run publo auth x login, and verify app/project access."
                    .to_string(),
            ),
            command: Some("publo auth x login".to_string()),
        });
    }
    if !status.is_success() {
        let maybe_json = response.json::<Value>().await.ok();
        return Err(AppError::Http {
            message: format!("X media upload returned {}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: maybe_json,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }

    let body = response
        .json::<XMediaUploadResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse X media upload response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })?;

    if let Some(errors) = body.errors {
        if !errors.is_empty() {
            return Err(AppError::Http {
                message: "X media upload returned errors.".to_string(),
                status: Some(200),
                api_error: Some(json!({ "errors": errors })),
                retryable: false,
            });
        }
    }

    let media_id = body
        .data
        .map(|d| d.id)
        .ok_or(AppError::Http {
            message: "X media upload response missing media id.".to_string(),
            status: Some(200),
            api_error: None,
            retryable: false,
        })?;

    Ok((media_id, None, None))
}

fn media_content_type(image_path: &PathBuf) -> Result<&'static str, AppError> {
    let ext = image_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        _ => Err(AppError::Validation {
            message: format!(
                "Unsupported media extension for '{}'. Allowed: .png, .jpg, .jpeg",
                image_path.display()
            ),
            suggestion: Some("Use PNG or JPEG image files.".to_string()),
            command: None,
        }),
    }
}

fn generate_state(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

fn generate_code_verifier(length: usize) -> String {
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

fn pkce_code_challenge(code_verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let digest = hasher.finalize();
    URL_SAFE_NO_PAD.encode(digest)
}

fn save_oauth_state(state: &OAuthState) -> Result<(), AppError> {
    let paths = config::resolve_runtime_paths();
    if let Some(parent) = paths.linkedin_oauth_state_path.parent() {
        fs::create_dir_all(parent).map_err(|err| AppError::Io {
            message: format!(
                "Failed to create OAuth state directory '{}': {err}",
                parent.display()
            ),
        })?;
    }
    let raw = serde_json::to_string(state).map_err(|err| AppError::Io {
        message: format!("Failed to encode OAuth state: {err}"),
    })?;
    fs::write(&paths.linkedin_oauth_state_path, raw).map_err(|err| AppError::Io {
        message: format!("Failed to write OAuth state file: {err}"),
    })
}

fn save_x_oauth_state(state: &XOAuthState) -> Result<(), AppError> {
    let paths = config::resolve_runtime_paths();
    if let Some(parent) = paths.x_oauth_state_path.parent() {
        fs::create_dir_all(parent).map_err(|err| AppError::Io {
            message: format!(
                "Failed to create X OAuth state directory '{}': {err}",
                parent.display()
            ),
        })?;
    }
    let raw = serde_json::to_string(state).map_err(|err| AppError::Io {
        message: format!("Failed to encode X OAuth state: {err}"),
    })?;
    fs::write(&paths.x_oauth_state_path, raw).map_err(|err| AppError::Io {
        message: format!("Failed to write X OAuth state file: {err}"),
    })
}

fn load_x_oauth_state() -> Result<XOAuthState, AppError> {
    let paths = config::resolve_runtime_paths();
    let raw = fs::read_to_string(&paths.x_oauth_state_path).map_err(|_| AppError::Validation {
        message: "X OAuth state file not found.".to_string(),
        suggestion: Some(
            "Run publo auth x login first, or pass --code-verifier to auth x exchange."
                .to_string(),
        ),
        command: Some("publo auth x login".to_string()),
    })?;
    serde_json::from_str::<XOAuthState>(&raw).map_err(|err| AppError::Io {
        message: format!("Failed to parse X OAuth state file: {err}"),
    })
}

fn clear_x_oauth_state_file() {
    let path = config::resolve_runtime_paths().x_oauth_state_path;
    let _ = fs::remove_file(path);
}

fn validate_oauth_state(input_state: &str) -> Result<(), AppError> {
    let paths = config::resolve_runtime_paths();
    let raw =
        fs::read_to_string(&paths.linkedin_oauth_state_path).map_err(|_| AppError::Validation {
        message: "OAuth state file not found. Start login again.".to_string(),
        suggestion: Some("Run publo auth linkedin login, then retry exchange with returned state.".to_string()),
        command: Some("publo auth linkedin login".to_string()),
    })?;
    let expected: OAuthState = serde_json::from_str(&raw).map_err(|err| AppError::Io {
        message: format!("Failed to parse OAuth state file: {err}"),
    })?;
    if input_state != expected.state {
        return Err(AppError::Validation {
            message: "OAuth state mismatch.".to_string(),
            suggestion: Some("Use the exact state value returned by login command.".to_string()),
            command: Some("publo auth linkedin login".to_string()),
        });
    }
    Ok(())
}

fn clear_oauth_state_file() {
    let path = config::resolve_runtime_paths().linkedin_oauth_state_path;
    let _ = fs::remove_file(path);
}

fn upsert_env_value(key: &str, value: &str) -> Result<(), AppError> {
    let env_path = config::resolve_runtime_paths().env_path;
    let mut lines = if let Ok(raw) = fs::read_to_string(&env_path) {
        raw.lines().map(|s| s.to_string()).collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut found = false;
    for line in &mut lines {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((existing_key, _)) = line.split_once('=') {
            if existing_key.trim() == key {
                *line = format!("{key}={}", format_env_value(value));
                found = true;
                break;
            }
        }
    }

    if !found {
        lines.push(format!("{key}={}", format_env_value(value)));
    }

    let output = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };

    if let Some(parent) = env_path.parent() {
        fs::create_dir_all(parent).map_err(|err| AppError::Io {
            message: format!("Failed to create .env directory '{}': {err}", parent.display()),
        })?;
    }

    fs::write(&env_path, output).map_err(|err| AppError::Io {
        message: format!("Failed to write .env: {err}"),
    })
}

fn format_env_value(value: &str) -> String {
    let needs_quotes = value.chars().any(|ch| ch.is_whitespace() || ch == '#' || ch == '"' || ch == '\'');
    if !needs_quotes {
        return value.to_string();
    }

    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod worker_tests {
    use super::*;

    struct TestWorkspace {
        root: PathBuf,
        config: RuntimeConfig,
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn test_workspace() -> TestWorkspace {
        let root = env::temp_dir().join(format!("publo-worker-test-{}", generate_id()));
        let workspace_dir = root.join("workspace");
        let runtime_dir = workspace_dir.join("runtime");
        fs::create_dir_all(&runtime_dir).expect("create test workspace");
        let db_path = workspace_dir.join("publo.db");
        let paths = RuntimePaths {
            publo_home: root.clone(),
            global_config_path: root.join("config.toml"),
            workspace_key: "worker-test".to_string(),
            workspace_dir: workspace_dir.clone(),
            workspace_config_path: workspace_dir.join("config.toml"),
            env_path: workspace_dir.join(".env"),
            runtime_dir: runtime_dir.clone(),
            db_default_path: db_path.clone(),
            publish_log_path: workspace_dir.join("publish-log.jsonl"),
            linkedin_oauth_state_path: runtime_dir.join("linkedin_oauth_state.json"),
            x_oauth_state_path: runtime_dir.join("x_oauth_state.json"),
        };
        let config = RuntimeConfig {
            workspace_id: generate_id(),
            pretty_json: false,
            connect_timeout: Duration::from_secs(1),
            request_timeout: Duration::from_secs(1),
            global_signature: SignatureLayer { enabled: None, text: None },
            linkedin_signature: SignatureLayer { enabled: None, text: None },
            x_signature: SignatureLayer { enabled: None, text: None },
            media_lookup_paths: Vec::new(),
            db_path,
            api_host: "127.0.0.1".to_string(),
            api_port: 0,
            catalog_roots: vec![workspace_dir.clone()],
            publish_cli_password: "test-password".to_string(),
            workspace_display_name: "Worker Test".to_string(),
            paths,
        };
        ensure_db_ready(&config).expect("migrate test database");
        TestWorkspace { root, config }
    }

    fn insert_due_job(workspace: &TestWorkspace, file_path: &PathBuf) -> String {
        let id = generate_id();
        let mut conn = open_db(&workspace.config).expect("open test database");
        sql_query(
            "INSERT INTO jobs (
                id, action_group_id, content_group_id, asset_id, kind, status, platform, workspace_id,
                selected_platforms, file_path, run_at_utc, created_at, updated_at
             ) VALUES (?, ?, ?, ?, 'catalog', 'scheduled', 'linkedin', ?, '[\"linkedin\"]', ?, '2020-01-01T00:00:00+00:00', ?, ?)",
        )
        .bind::<Text, _>(&id)
        .bind::<Text, _>(generate_id())
        .bind::<Text, _>(generate_id())
        .bind::<Text, _>(generate_id())
        .bind::<Text, _>(&workspace.config.workspace_id)
        .bind::<Text, _>(file_path.display().to_string())
        .bind::<Text, _>(now_rfc3339_utc())
        .bind::<Text, _>(now_rfc3339_utc())
        .execute(&mut conn)
        .expect("insert due job");
        id
    }

    fn write_post(workspace: &TestWorkspace, name: &str) -> PathBuf {
        let path = workspace.config.paths.workspace_dir.join(name);
        fs::write(&path, "# Test post\n\nA local worker test post.").expect("write post");
        path
    }

    fn attempt_rows(conn: &mut SqliteConnection, job_id: &str) -> Vec<TestAttemptRow> {
        sql_query(
            "SELECT success, error_type, post_id
             FROM publish_attempts WHERE job_id = ? ORDER BY attempt_no ASC",
        )
        .bind::<Text, _>(job_id)
        .load(conn)
        .expect("read attempts")
    }

    #[derive(Debug, QueryableByName)]
    struct TestAttemptRow {
        #[diesel(sql_type = Integer)]
        success: i32,
        #[diesel(sql_type = Nullable<Text>)]
        error_type: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        post_id: Option<String>,
    }

    enum FakePublishOutcome {
        Success,
        Failure,
    }

    struct FakeWorkerPublisher {
        outcome: FakePublishOutcome,
    }

    impl WorkerPublisher for FakeWorkerPublisher {
        fn preflight(&self, job: &JobRow) -> Result<JobPreflightResult, AppError> {
            let raw = fs::read_to_string(&job.file_path).map_err(|err| AppError::Io {
                message: format!("Fake worker could not read {}: {err}", job.file_path),
            })?;
            Ok(JobPreflightResult {
                file_sha256: compute_content_sha256(raw.as_bytes()),
                text_sha256: compute_content_sha256(raw.as_bytes()),
                fingerprint: format!("fake:{}", job.id),
                details: json!({ "fake": true }),
            })
        }

        fn publish(
            &self,
            _job: &JobRow,
            _preflight: &JobPreflightResult,
        ) -> Result<WorkerPublishReceipt, AppError> {
            match self.outcome {
                FakePublishOutcome::Success => Ok(WorkerPublishReceipt {
                    post_id: Some("fake-post-1".to_string()),
                    post_url: Some("https://example.test/posts/fake-post-1".to_string()),
                    request_id: Some("fake-request-1".to_string()),
                    response: json!({ "fake": true, "published": true }),
                }),
                FakePublishOutcome::Failure => Err(AppError::Http {
                    message: "Fake provider temporary failure.".to_string(),
                    status: Some(503),
                    api_error: None,
                    retryable: true,
                }),
            }
        }
    }

    #[test]
    fn worker_claim_and_fake_publish_records_success() {
        let workspace = test_workspace();
        let file = write_post(&workspace, "success.md");
        let job_id = insert_due_job(&workspace, &file);
        let now = now_rfc3339_utc();
        let mut conn = open_db(&workspace.config).expect("open database");
        let job = claim_due_job(&mut conn, &job_id, &now)
            .expect("claim job")
            .expect("job is claimed");

        execute_claimed_job(
            &mut conn,
            &job.job,
            &job.claim_token,
            &FakeWorkerPublisher { outcome: FakePublishOutcome::Success },
            &now,
        )
        .expect("execute fake publish");

        let saved = get_job_by_id(&mut conn, &job_id).expect("read published job");
        let attempts = attempt_rows(&mut conn, &job_id);
        assert_eq!(saved.status, "published");
        assert_eq!(saved.attempt_count, 1);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].success, 1);
        assert_eq!(attempts[0].post_id.as_deref(), Some("fake-post-1"));
    }

    #[test]
    fn worker_blocks_when_preflight_cannot_read_file() {
        let workspace = test_workspace();
        let missing_file = workspace.config.paths.workspace_dir.join("missing.md");
        let job_id = insert_due_job(&workspace, &missing_file);
        let now = now_rfc3339_utc();
        let mut conn = open_db(&workspace.config).expect("open database");
        let job = claim_due_job(&mut conn, &job_id, &now)
            .expect("claim job")
            .expect("job is claimed");

        execute_claimed_job(
            &mut conn,
            &job.job,
            &job.claim_token,
            &FakeWorkerPublisher { outcome: FakePublishOutcome::Success },
            &now,
        )
        .expect("record blocked result");

        let saved = get_job_by_id(&mut conn, &job_id).expect("read blocked job");
        let attempts = attempt_rows(&mut conn, &job_id);
        assert_eq!(saved.status, "blocked");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].success, 0);
        assert_eq!(attempts[0].error_type.as_deref(), Some("io_error"));
    }

    #[test]
    fn worker_records_publish_failures_without_publishing() {
        let workspace = test_workspace();
        let file = write_post(&workspace, "failure.md");
        let job_id = insert_due_job(&workspace, &file);
        let now = now_rfc3339_utc();
        let mut conn = open_db(&workspace.config).expect("open database");
        let job = claim_due_job(&mut conn, &job_id, &now)
            .expect("claim job")
            .expect("job is claimed");

        execute_claimed_job(
            &mut conn,
            &job.job,
            &job.claim_token,
            &FakeWorkerPublisher { outcome: FakePublishOutcome::Failure },
            &now,
        )
        .expect("record failed result");

        let saved = get_job_by_id(&mut conn, &job_id).expect("read failed job");
        let attempts = attempt_rows(&mut conn, &job_id);
        assert_eq!(saved.status, "failed");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].success, 0);
        assert_eq!(attempts[0].error_type.as_deref(), Some("http_error"));
    }

    #[test]
    fn worker_claim_is_atomic_and_creates_one_attempt() {
        let workspace = test_workspace();
        let file = write_post(&workspace, "claim.md");
        let job_id = insert_due_job(&workspace, &file);
        let now = now_rfc3339_utc();
        let mut first = open_db(&workspace.config).expect("open first connection");
        let mut second = open_db(&workspace.config).expect("open second connection");

        assert!(claim_due_job(&mut first, &job_id, &now)
            .expect("first claim")
            .is_some());
        assert!(claim_due_job(&mut second, &job_id, &now)
            .expect("second claim")
            .is_none());
        assert_eq!(attempt_rows(&mut first, &job_id).len(), 1);
    }

    #[test]
    fn interrupted_publish_is_not_automatically_retried() {
        let workspace = test_workspace();
        let file = write_post(&workspace, "interrupted.md");
        let job_id = insert_due_job(&workspace, &file);
        let now = now_rfc3339_utc();
        let mut first = open_db(&workspace.config).expect("open first connection");
        let claimed = claim_due_job(&mut first, &job_id, &now)
            .expect("claim job")
            .expect("job is claimed");

        let publisher = FakeWorkerPublisher { outcome: FakePublishOutcome::Success };
        let preflight = publisher.preflight(&claimed.job).expect("fake preflight");
        let _receipt = publisher.publish(&claimed.job, &preflight).expect("provider accepted post");
        drop(first); // Simulate process death before Publo records completion.

        let mut restarted = open_db(&workspace.config).expect("open restarted connection");
        assert!(claim_due_job(&mut restarted, &job_id, &now)
            .expect("retry claim")
            .is_none());
        let saved = get_job_by_id(&mut restarted, &job_id).expect("read stranded job");
        assert_eq!(saved.status, "publishing");
        assert_eq!(attempt_rows(&mut restarted, &job_id).len(), 1);
    }

    #[test]
    fn expired_claim_is_reported_then_reconciled_as_blocked() {
        let workspace = test_workspace();
        let file = write_post(&workspace, "expired-claim.md");
        let job_id = insert_due_job(&workspace, &file);
        let now = now_rfc3339_utc();
        let mut conn = open_db(&workspace.config).expect("open database");
        let _claim = claim_due_job(&mut conn, &job_id, &now)
            .expect("claim job")
            .expect("job is claimed");
        let expired_at = (Utc::now() - chrono::Duration::minutes(6)).to_rfc3339();
        sql_query("UPDATE jobs SET publishing_started_at = ? WHERE id = ?")
            .bind::<Text, _>(&expired_at)
            .bind::<Text, _>(&job_id)
            .execute(&mut conn)
            .expect("age claim");

        let dry_output = worker_run_dry_once(
            WorkerRunArgs { dry_run: true, once: true },
            &workspace.config,
        )
        .expect("dry run");
        assert_eq!(dry_output["interrupted_count"].as_u64(), Some(1));
        assert_eq!(get_job_by_id(&mut conn, &job_id).expect("read job").status, "publishing");

        let reconciled = reconcile_expired_worker_claims(&mut conn, &now).expect("reconcile claim");
        assert_eq!(reconciled.len(), 1);
        let saved = get_job_by_id(&mut conn, &job_id).expect("read blocked job");
        let attempts = attempt_rows(&mut conn, &job_id);
        assert_eq!(saved.status, "blocked");
        assert_eq!(attempts[0].success, 0);
        assert_eq!(attempts[0].error_type.as_deref(), Some("worker_interrupted"));
    }

    #[test]
    fn dry_run_does_not_claim_or_create_attempts() {
        let workspace = test_workspace();
        let file = write_post(&workspace, "dry-run.md");
        let job_id = insert_due_job(&workspace, &file);

        worker_run_dry_once(
            WorkerRunArgs { dry_run: true, once: true },
            &workspace.config,
        )
        .expect("run dry worker");

        let mut conn = open_db(&workspace.config).expect("open database");
        let saved = get_job_by_id(&mut conn, &job_id).expect("read scheduled job");
        assert_eq!(saved.status, "scheduled");
        assert_eq!(saved.attempt_count, 0);
        assert!(attempt_rows(&mut conn, &job_id).is_empty());
    }

    #[test]
    fn database_rejects_rows_for_another_workspace() {
        let workspace = test_workspace();
        let file = write_post(&workspace, "other-workspace.md");
        let job_id = insert_due_job(&workspace, &file);
        let mut conn = open_db(&workspace.config).expect("open database");
        let result = sql_query("UPDATE jobs SET workspace_id = ? WHERE id = ?")
            .bind::<Text, _>(generate_id())
            .bind::<Text, _>(&job_id)
            .execute(&mut conn)
            ;
        assert!(result.is_err());
        assert!(attempt_rows(&mut conn, &job_id).is_empty());
    }
}
