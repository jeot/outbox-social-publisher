use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use rand::Rng;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{oneshot, Mutex};
use url::Url;
use urlencoding::encode;

const DEFAULT_LINKEDIN_VERSION: &str = "202601";
const OAUTH_STATE_PATH: &str = ".outbox/linkedin_oauth_state";
const PUBLISH_LOG_PATH: &str = ".outbox/publish-log.jsonl";
const ENV_PATH: &str = ".env";
const DEFAULT_X_SCOPES: &str = "tweet.read tweet.write users.read offline.access";

#[derive(Debug, Parser)]
#[command(name = "outbox")]
#[command(about = "Local-first publishing CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Publish(PublishArgs),
    Auth(AuthArgs),
}

#[derive(Debug, Args)]
struct PublishArgs {
    #[command(subcommand)]
    platform: PublishPlatform,
}

#[derive(Debug, Subcommand)]
enum PublishPlatform {
    Linkedin(PublishLinkedinArgs),
    X(PublishXArgs),
}

#[derive(Debug, Args)]
struct PublishLinkedinArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    timeout_seconds: Option<u64>,
    #[arg(long, default_value_t = false)]
    allow_duplicate: bool,
    #[arg(long, default_value_t = false)]
    debug: bool,
    #[arg(long, default_value_t = false, conflicts_with = "no_signature")]
    add_signature: bool,
    #[arg(long, default_value_t = false, conflicts_with = "add_signature")]
    no_signature: bool,
}

#[derive(Debug, Args)]
struct PublishXArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    timeout_seconds: Option<u64>,
    #[arg(long, default_value_t = false)]
    allow_duplicate: bool,
    #[arg(long, default_value_t = false)]
    allow_cashtag: bool,
    #[arg(long, default_value_t = false)]
    allow_length: bool,
    #[arg(long, default_value_t = false)]
    force: bool,
    #[arg(long, default_value_t = false)]
    debug: bool,
    #[arg(long, default_value_t = false, conflicts_with = "no_signature")]
    add_signature: bool,
    #[arg(long, default_value_t = false, conflicts_with = "add_signature")]
    no_signature: bool,
}

#[derive(Debug, Args)]
struct AuthArgs {
    #[command(subcommand)]
    platform: AuthPlatform,
}

#[derive(Debug, Subcommand)]
enum AuthPlatform {
    Linkedin(AuthLinkedinArgs),
    X(AuthXArgs),
}

#[derive(Debug, Args)]
struct AuthLinkedinArgs {
    #[command(subcommand)]
    command: AuthLinkedinCommand,
}

#[derive(Debug, Subcommand)]
enum AuthLinkedinCommand {
    Guide,
    Login,
    Exchange(AuthLinkedinExchangeArgs),
    Whoami,
    TokenStatus,
    TokenRefresh,
}

#[derive(Debug, Args)]
struct AuthXArgs {
    #[command(subcommand)]
    command: AuthXCommand,
}

#[derive(Debug, Subcommand)]
enum AuthXCommand {
    Login,
}

#[derive(Debug, Args)]
struct AuthLinkedinExchangeArgs {
    #[arg(long)]
    code: String,
    #[arg(long)]
    state: String,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    output: Option<OutputConfig>,
    timeouts: Option<TimeoutConfig>,
    signature: Option<SignatureConfigFile>,
    platform: Option<PlatformConfig>,
    media: Option<MediaConfigFile>,
}

#[derive(Debug, Deserialize)]
struct OutputConfig {
    pretty_json: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TimeoutConfig {
    connect_seconds: Option<u64>,
    request_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
struct SignatureConfigFile {
    enabled: Option<bool>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlatformConfig {
    linkedin: Option<PlatformEntryConfig>,
    x: Option<PlatformEntryConfig>,
}

#[derive(Debug, Deserialize)]
struct PlatformEntryConfig {
    signature: Option<SignatureConfigFile>,
}

#[derive(Debug, Deserialize)]
struct MediaConfigFile {
    lookup_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct SignatureLayer {
    enabled: Option<bool>,
    text: Option<String>,
}

#[derive(Debug)]
struct RuntimeConfig {
    pretty_json: bool,
    connect_timeout: Duration,
    request_timeout: Duration,
    global_signature: SignatureLayer,
    linkedin_signature: SignatureLayer,
    x_signature: SignatureLayer,
    media_lookup_paths: Vec<PathBuf>,
}

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

#[derive(Debug, Serialize)]
struct ErrorOutput {
    ok: bool,
    error_type: &'static str,
    message: String,
    http_status: Option<u16>,
    api_error: Option<Value>,
    retryable: bool,
    suggestion: Option<String>,
    command: Option<String>,
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

#[derive(Debug)]
enum AppError {
    Validation {
        message: String,
        suggestion: Option<String>,
        command: Option<String>,
    },
    MissingAuth {
        message: String,
        suggestion: Option<String>,
        command: Option<String>,
    },
    Io {
        message: String,
    },
    Http {
        message: String,
        status: Option<u16>,
        api_error: Option<Value>,
        retryable: bool,
    },
    DuplicatePublish {
        message: String,
        existing_post_id: Option<String>,
        existing_post_url: Option<String>,
        file_sha256: String,
        fingerprint: String,
        existing_published_at: String,
    },
}

impl AppError {
    fn exit_code(&self) -> u8 {
        match self {
            AppError::Validation { .. } => 2,
            AppError::MissingAuth { .. } => 3,
            AppError::Io { .. } => 4,
            AppError::Http { .. } => 5,
            AppError::DuplicatePublish { .. } => 6,
        }
    }

    fn to_output(&self) -> ErrorOutput {
        match self {
            AppError::Validation {
                message,
                suggestion,
                command,
            } => ErrorOutput {
                ok: false,
                error_type: "validation_error",
                message: message.clone(),
                http_status: None,
                api_error: None,
                retryable: false,
                suggestion: suggestion.clone(),
                command: command.clone(),
            },
            AppError::MissingAuth {
                message,
                suggestion,
                command,
            } => ErrorOutput {
                ok: false,
                error_type: "missing_auth",
                message: message.clone(),
                http_status: None,
                api_error: None,
                retryable: false,
                suggestion: suggestion.clone(),
                command: command.clone(),
            },
            AppError::Io { message } => ErrorOutput {
                ok: false,
                error_type: "io_error",
                message: message.clone(),
                http_status: None,
                api_error: None,
                retryable: false,
                suggestion: None,
                command: None,
            },
            AppError::Http {
                message,
                status,
                api_error,
                retryable,
            } => ErrorOutput {
                ok: false,
                error_type: "http_error",
                message: message.clone(),
                http_status: *status,
                api_error: api_error.clone(),
                retryable: *retryable,
                suggestion: Some(http_error_suggestion(*status, api_error.as_ref())),
                command: None,
            },
            AppError::DuplicatePublish {
                message,
                existing_post_id,
                existing_post_url,
                file_sha256,
                fingerprint,
                existing_published_at,
            } => ErrorOutput {
                ok: false,
                error_type: "duplicate_publish",
                message: message.clone(),
                http_status: None,
                api_error: Some(json!({
                    "existing_post_id": existing_post_id,
                    "existing_post_url": existing_post_url,
                    "file_sha256": file_sha256,
                    "fingerprint": fingerprint,
                    "existing_published_at": existing_published_at
                })),
                retryable: false,
                suggestion: Some("Use --allow-duplicate to bypass duplicate guard intentionally.".to_string()),
                command: None,
            },
        }
    }
}

fn http_error_suggestion(status: Option<u16>, api_error: Option<&Value>) -> String {
    if let Some(err) = api_error {
        if let Some(hint) = err.get("local_hint").and_then(|v| v.as_str()) {
            if hint == "x_likely_over_length" {
                let weighted = err
                    .get("local_weighted_length")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                return format!(
                    "X API returned generic forbidden, but local check indicates over length (weighted {} > 280). Shorten text and retry.",
                    weighted
                );
            }
            if hint == "x_likely_cashtag_limit" {
                let count = err
                    .get("local_cashtag_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                return format!(
                    "X API returned generic forbidden, but local check indicates cashtag limit ({} found; API self-serve allows max 1). Reduce cashtags and retry.",
                    count
                );
            }
        }
    }

    if let Some(402) = status {
        if let Some(err) = api_error {
            let typ = err.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            let detail = err
                .get("detail")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if typ.contains("credits-depleted") || detail.contains("credits depleted") {
                return "X API credits are depleted for this app/project. Enable billing or upgrade access in X Developer Portal, then retry publish.".to_string();
            }
        }
    }

    if let Some(403) = status {
        if let Some(err) = api_error {
            let detail_raw = err
                .get("detail")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let detail = detail_raw.to_ascii_lowercase();
            let typ = err.get("type").and_then(|v| v.as_str()).unwrap_or_default();

            if detail.contains("maximum of one cashtag")
                || detail.contains("remove additional cashtags")
            {
                return "X API rejected the post due to cashtag limit (max 1 cashtag in API self-serve mode). Reduce cashtags to one and retry.".to_string();
            }

            if detail.contains("too long")
                || detail.contains("over 280")
                || detail.contains("280")
            {
                return "X API rejected post length. Shorten text to fit API weighted 280-character rules, then retry.".to_string();
            }

            if typ.contains("not-authorized-for-resource") {
                return "Request is not authorized for this API resource. Verify app/project access level and OAuth scopes for this endpoint.".to_string();
            }
        }
        return "Request is forbidden by provider policy/settings. Verify app permissions, user scopes, and product access for this endpoint.".to_string();
    }

    "Inspect api_error for provider details and retry when resolved.".to_string()
}

#[derive(Debug)]
struct LinkedinAuth {
    access_token: String,
    refresh_token: Option<String>,
    author_urn: String,
    version: String,
}

#[derive(Debug)]
struct XAuth {
    access_token: String,
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

#[tokio::main]
async fn main() -> ExitCode {
    let _ = dotenvy::from_filename_override(".env");
    let config = load_config();
    let cli = Cli::parse();

    let result: Result<Value, AppError> = match cli.command {
        Commands::Publish(publish) => match publish.platform {
            PublishPlatform::Linkedin(args) => publish_linkedin(args, &config).await,
            PublishPlatform::X(args) => publish_x(args, &config).await,
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
            },
        },
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

fn load_config() -> RuntimeConfig {
    let defaults = RuntimeConfig {
        pretty_json: false,
        connect_timeout: Duration::from_secs(10),
        request_timeout: Duration::from_secs(30),
        global_signature: SignatureLayer {
            enabled: None,
            text: None,
        },
        linkedin_signature: SignatureLayer {
            enabled: None,
            text: None,
        },
        x_signature: SignatureLayer {
            enabled: None,
            text: None,
        },
        media_lookup_paths: Vec::new(),
    };

    let Ok(raw) = fs::read_to_string("config.toml") else {
        return defaults;
    };

    let Ok(file_config) = toml::from_str::<FileConfig>(&raw) else {
        return defaults;
    };

    let pretty_json = file_config
        .output
        .and_then(|o| o.pretty_json)
        .unwrap_or(defaults.pretty_json);

    let connect_timeout = file_config
        .timeouts
        .as_ref()
        .and_then(|t| t.connect_seconds)
        .map(Duration::from_secs)
        .unwrap_or(defaults.connect_timeout);

    let request_timeout = file_config
        .timeouts
        .as_ref()
        .and_then(|t| t.request_seconds)
        .map(Duration::from_secs)
        .unwrap_or(defaults.request_timeout);

    let global_signature = to_signature_layer(file_config.signature.as_ref());
    let linkedin_signature = to_signature_layer(
        file_config
            .platform
            .as_ref()
            .and_then(|p| p.linkedin.as_ref())
            .and_then(|p| p.signature.as_ref()),
    );
    let x_signature = to_signature_layer(
        file_config
            .platform
            .as_ref()
            .and_then(|p| p.x.as_ref())
            .and_then(|p| p.signature.as_ref()),
    );
    let media_lookup_paths = file_config
        .media
        .as_ref()
        .and_then(|m| m.lookup_paths.as_ref())
        .map(|items| items.iter().map(PathBuf::from).collect())
        .unwrap_or_else(Vec::new);

    RuntimeConfig {
        pretty_json,
        connect_timeout,
        request_timeout,
        global_signature,
        linkedin_signature,
        x_signature,
        media_lookup_paths,
    }
}

fn to_signature_layer(cfg: Option<&SignatureConfigFile>) -> SignatureLayer {
    SignatureLayer {
        enabled: cfg.and_then(|c| c.enabled),
        text: cfg.and_then(|c| c.text.clone()),
    }
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
            "command": "outbox auth linkedin guide"
        })
    } else if access_token.is_none() {
        json!({
            "message": "No LinkedIn access token found. Start OAuth login.",
            "command": "outbox auth linkedin login",
            "scopes": scopes
        })
    } else if author_urn.is_none() {
        json!({
            "message": "Access token exists but LINKEDIN_AUTHOR_URN is missing.",
            "required_env": ["LINKEDIN_AUTHOR_URN"],
            "example": "urn:li:person:xxxxxxxx",
            "command": "outbox auth linkedin whoami"
        })
    } else {
        json!({
            "message": "LinkedIn auth appears ready.",
            "command": "outbox publish linkedin --file <path>"
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
        command: Some("outbox auth x login".to_string()),
    })?;
    let redirect_uri = env_non_empty("X_REDIRECT_URI").ok_or(AppError::MissingAuth {
        message: "X_REDIRECT_URI is missing.".to_string(),
        suggestion: Some("Set X_REDIRECT_URI in .env and match it in your X app callback URL settings.".to_string()),
        command: Some("outbox auth x login".to_string()),
    })?;
    let configured_scopes = env_non_empty("X_SCOPES").unwrap_or_else(|| DEFAULT_X_SCOPES.to_string());
    let scopes = ensure_x_required_scopes(&configured_scopes);
    let client_secret = env_non_empty("X_CLIENT_SECRET");

    let redirect = Url::parse(&redirect_uri).map_err(|err| AppError::Validation {
        message: format!("Invalid X_REDIRECT_URI: {err}"),
        suggestion: Some("Use a full URL like http://127.0.0.1:8789/callback".to_string()),
        command: Some("outbox auth x login".to_string()),
    })?;
    let host = redirect.host_str().ok_or(AppError::Validation {
        message: "X_REDIRECT_URI must include a host.".to_string(),
        suggestion: Some("Use localhost or 127.0.0.1 callback URL.".to_string()),
        command: Some("outbox auth x login".to_string()),
    })?;
    let port = redirect.port().unwrap_or(80);
    let path = redirect.path().to_string();

    let state = generate_state(32);
    let code_verifier = generate_code_verifier(64);
    let code_challenge = pkce_code_challenge(&code_verifier);

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
            command: Some("outbox auth x login".to_string()),
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
                "command": "outbox publish x --file <path>"
            }
        })),
        Err(err) => Err(err),
    }
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
            command: Some("outbox auth x login".to_string()),
        });
    }

    let code = query.code.ok_or(AppError::Validation {
        message: "X callback did not include authorization code.".to_string(),
        suggestion: Some("Retry auth flow and complete consent.".to_string()),
        command: Some("outbox auth x login".to_string()),
    })?;
    let returned_state = query.state.ok_or(AppError::Validation {
        message: "X callback did not include state.".to_string(),
        suggestion: Some("Retry auth flow.".to_string()),
        command: Some("outbox auth x login".to_string()),
    })?;
    if returned_state != state.expected_state {
        return Err(AppError::Validation {
            message: "X OAuth state mismatch.".to_string(),
            suggestion: Some("Retry auth flow; ensure you use the latest auth URL.".to_string()),
            command: Some("outbox auth x login".to_string()),
        });
    }

    exchange_x_code_for_token(
        code,
        &state.client_id,
        state.client_secret.as_deref(),
        &state.redirect_uri,
        &state.code_verifier,
        state.connect_timeout,
        state.request_timeout,
    )
    .await
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

async fn start_linkedin_login(config: &RuntimeConfig) -> Result<Value, AppError> {
    let client_id = env::var("LINKEDIN_CLIENT_ID").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_ID is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_ID in .env from your LinkedIn app settings.".to_string()),
        command: Some("outbox auth linkedin guide".to_string()),
    })?;
    let redirect_uri = env::var("LINKEDIN_REDIRECT_URI").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_REDIRECT_URI is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_REDIRECT_URI in .env and match it in LinkedIn app settings.".to_string()),
        command: Some("outbox auth linkedin guide".to_string()),
    })?;
    let scopes = env::var("LINKEDIN_SCOPES").unwrap_or_else(|_| "w_member_social".to_string());
    let client_secret = env::var("LINKEDIN_CLIENT_SECRET").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_SECRET is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_SECRET in .env from your LinkedIn app settings.".to_string()),
        command: Some("outbox auth linkedin guide".to_string()),
    })?;

    let redirect = Url::parse(&redirect_uri).map_err(|err| AppError::Validation {
        message: format!("Invalid LINKEDIN_REDIRECT_URI: {err}"),
        suggestion: Some("Use a full URL like http://localhost:8788/callback".to_string()),
        command: Some("outbox auth linkedin login".to_string()),
    })?;
    let host = redirect.host_str().ok_or(AppError::Validation {
        message: "LINKEDIN_REDIRECT_URI must include a host.".to_string(),
        suggestion: Some("Use localhost or 127.0.0.1 callback URL.".to_string()),
        command: Some("outbox auth linkedin login".to_string()),
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
            command: Some("outbox auth linkedin login".to_string()),
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
                "command": "outbox publish linkedin --file <path>"
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
            command: Some("outbox auth linkedin login".to_string()),
        });
    }

    let code = query.code.ok_or(AppError::Validation {
        message: "LinkedIn callback did not include authorization code.".to_string(),
        suggestion: Some("Retry auth flow and complete consent.".to_string()),
        command: Some("outbox auth linkedin login".to_string()),
    })?;
    let returned_state = query.state.ok_or(AppError::Validation {
        message: "LinkedIn callback did not include state.".to_string(),
        suggestion: Some("Retry auth flow.".to_string()),
        command: Some("outbox auth linkedin login".to_string()),
    })?;
    if returned_state != state.expected_state {
        return Err(AppError::Validation {
            message: "LinkedIn OAuth state mismatch.".to_string(),
            suggestion: Some("Retry auth flow; ensure you use the latest auth URL.".to_string()),
            command: Some("outbox auth linkedin login".to_string()),
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
        command: Some("outbox auth linkedin guide".to_string()),
    })?;
    let client_secret = env::var("LINKEDIN_CLIENT_SECRET").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_SECRET is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_SECRET in .env from your LinkedIn app settings.".to_string()),
        command: Some("outbox auth linkedin guide".to_string()),
    })?;
    let redirect_uri = env::var("LINKEDIN_REDIRECT_URI").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_REDIRECT_URI is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_REDIRECT_URI in .env and match it in LinkedIn app settings.".to_string()),
        command: Some("outbox auth linkedin guide".to_string()),
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
            "command": "outbox auth linkedin whoami"
        }
    }))
}

async fn resolve_linkedin_author_urn(config: &RuntimeConfig) -> Result<Value, AppError> {
    let access_token = env::var("LINKEDIN_ACCESS_TOKEN").map_err(|_| AppError::MissingAuth {
        message: "No LinkedIn access token found.".to_string(),
        suggestion: Some(
            "Run auth flow first: outbox auth linkedin login, then exchange.".to_string(),
        ),
        command: Some("outbox auth linkedin guide".to_string()),
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
            "command": "outbox publish linkedin --file <path>"
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
        command: Some("outbox auth linkedin token-status".to_string()),
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
            "command": "outbox auth linkedin token-status"
        }
    }))
}

async fn publish_linkedin(args: PublishLinkedinArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    if !args.file.exists() {
        return Err(AppError::Validation {
            message: format!("Content file does not exist: {}", args.file.display()),
            suggestion: Some("Check the file path and run the same command again.".to_string()),
            command: None,
        });
    }

    let parsed = parse_post_input(&args.file, config)?;
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
            if parsed.media_paths.len() > 20 {
                return Err(AppError::Validation {
                    message: format!(
                        "LinkedIn multi-image supports at most 20 images; found {}.",
                        parsed.media_paths.len()
                    ),
                    suggestion: Some("Reduce image count to 20 or fewer and retry.".to_string()),
                    command: None,
                });
            }
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
                command: Some("outbox auth linkedin guide".to_string()),
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
        if parsed.media_paths.len() > 20 {
            return Err(AppError::Validation {
                message: format!(
                    "LinkedIn multi-image supports at most 20 images; found {}.",
                    parsed.media_paths.len()
                ),
                suggestion: Some("Reduce image count to 20 or fewer and retry.".to_string()),
                command: None,
            });
        }
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
                        command: Some("outbox auth linkedin login".to_string()),
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

async fn publish_x(args: PublishXArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    if !args.file.exists() {
        return Err(AppError::Validation {
            message: format!("Content file does not exist: {}", args.file.display()),
            suggestion: Some("Check the file path and run the same command again.".to_string()),
            command: None,
        });
    }

    let parsed = parse_post_input(&args.file, config)?;
    let mut text = parsed.publish_text;
    let signature_override = signature_cli_override(args.add_signature, args.no_signature);
    let mut signature_applied = false;
    if let Some(signature) =
        resolve_signature_text(config, PublishPlatformKind::X, signature_override)?
    {
        text.push_str(&signature);
        signature_applied = true;
    }
    if !parsed.media_paths.is_empty() {
        return Err(AppError::Validation {
            message: format!(
                "X media publish is not implemented yet ({} images found).",
                parsed.media_paths.len()
            ),
            suggestion: Some(
                "Remove image embeds for X publish for now, or publish to LinkedIn where single-image upload is supported."
                    .to_string(),
            ),
            command: None,
        });
    }
    let bypass_duplicate = args.force || args.allow_duplicate;
    let bypass_cashtag = args.force || args.allow_cashtag;
    let bypass_length = args.force || args.allow_length;
    let cashtag_count = extract_cashtags(&text).len();
    let weighted_len = x_weighted_length(&text);

    validate_x_post_text(&text, bypass_cashtag, bypass_length)?;

    let auth = load_x_auth()?;
    let author_key = x_author_key();
    let media_sha = compute_media_signature(&parsed.media_paths)?;
    let fingerprint_source = combine_text_and_media_for_fingerprint(&text, &media_sha);
    let file_sha256 = parsed.file_sha256;
    let text_sha256 = compute_content_sha256(text.as_bytes());
    let fingerprint = compute_fingerprint("x", &author_key, &fingerprint_source);
    maybe_block_duplicate("x", &author_key, &fingerprint, &file_sha256, bypass_duplicate)?;

    if args.debug {
        return Ok(json!({
            "ok": true,
            "platform": "x",
            "mode": "debug",
            "would_publish": true,
            "signature_applied": signature_applied,
            "media_count": parsed.media_paths.len(),
            "media_paths": parsed.media_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "text": text,
            "payload": {
                "text": text
            },
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

    let response = client
        .post("https://api.x.com/2/tweets")
        .header(AUTHORIZATION, format!("Bearer {}", auth.access_token))
        .header(CONTENT_TYPE, "application/json")
        .json(&json!({ "text": text }))
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

    if !status.is_success() {
        let mut maybe_json = response.json::<Value>().await.ok();
        if status.as_u16() == 403 && is_generic_x_forbidden(maybe_json.as_ref()) {
            if weighted_len > 280 {
                let mut err = maybe_json.unwrap_or_else(|| json!({}));
                if !err.is_object() {
                    err = json!({ "provider_error": err });
                }
                if let Some(obj) = err.as_object_mut() {
                    obj.insert(
                        "local_hint".to_string(),
                        json!("x_likely_over_length"),
                    );
                    obj.insert(
                        "local_weighted_length".to_string(),
                        json!(weighted_len),
                    );
                    obj.insert("local_weighted_limit".to_string(), json!(280));
                }
                maybe_json = Some(err);
            } else if cashtag_count > 1 {
                let mut err = maybe_json.unwrap_or_else(|| json!({}));
                if !err.is_object() {
                    err = json!({ "provider_error": err });
                }
                if let Some(obj) = err.as_object_mut() {
                    obj.insert(
                        "local_hint".to_string(),
                        json!("x_likely_cashtag_limit"),
                    );
                    obj.insert("local_cashtag_count".to_string(), json!(cashtag_count));
                    obj.insert("local_cashtag_limit".to_string(), json!(1));
                }
                maybe_json = Some(err);
            }
        }
        if status.as_u16() == 401 {
            return Err(AppError::MissingAuth {
                message: "X access token is invalid or expired.".to_string(),
                suggestion: Some("Update X_ACCESS_TOKEN in .env and retry publish.".to_string()),
                command: Some("outbox publish x --file <path>".to_string()),
            });
        }
        return Err(AppError::Http {
            message: format!("X API returned {}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: maybe_json,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }

    let body = response.json::<Value>().await.ok();
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

fn validate_x_post_text(
    text: &str,
    allow_cashtag: bool,
    allow_length: bool,
) -> Result<(), AppError> {
    let cashtags = extract_cashtags(text);
    if !allow_cashtag && cashtags.len() > 1 {
        return Err(AppError::Validation {
            message: format!(
                "X self-serve posting allows max 1 cashtag per post; found {}.",
                cashtags.len()
            ),
            suggestion: Some(
                "Keep at most one cashtag (for example: $AAPL), or use --allow-cashtag to bypass local check."
                    .to_string(),
            ),
            command: Some("outbox publish x --file <path>".to_string()),
        });
    }

    let weighted_len = x_weighted_length(text);
    if !allow_length && weighted_len > 280 {
        return Err(AppError::Validation {
            message: format!(
                "X post is too long by weighted count: {} > 280.",
                weighted_len
            ),
            suggestion: Some(
                "Shorten text (URLs count as 23 chars; many non-ASCII/emoji chars count as 2), or use --allow-length to bypass local check."
                    .to_string(),
            ),
            command: Some("outbox publish x --file <path>".to_string()),
        });
    }

    Ok(())
}

fn extract_cashtags(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] != '$' {
            i += 1;
            continue;
        }

        let prev_is_word = if i == 0 {
            false
        } else {
            let p = chars[i - 1];
            p.is_ascii_alphanumeric() || p == '_'
        };
        if prev_is_word {
            i += 1;
            continue;
        }

        let mut j = i + 1;
        if j >= chars.len() || !chars[j].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        j += 1;
        while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
            j += 1;
        }

        let next_is_word = if j < chars.len() {
            let n = chars[j];
            n.is_ascii_alphanumeric() || n == '_'
        } else {
            false
        };
        if !next_is_word {
            let tag: String = chars[i..j].iter().collect();
            out.push(tag);
        }
        i = j;
    }

    out
}

fn x_weighted_length(text: &str) -> usize {
    text.split_whitespace()
        .map(|token| {
            if looks_like_url(token) {
                23
            } else {
                token.chars().map(x_char_weight).sum()
            }
        })
        .sum::<usize>()
        + text.chars().filter(|c| c.is_whitespace()).count()
}

fn looks_like_url(token: &str) -> bool {
    let stripped = token
        .trim_end_matches(|c: char| ",.!?;:)]}\"'".contains(c))
        .trim_start_matches('(')
        .trim_start_matches('[')
        .trim_start_matches('{');
    let lower = stripped.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://")) && Url::parse(stripped).is_ok()
}

fn x_char_weight(ch: char) -> usize {
    if ch.is_ascii() {
        return 1;
    }
    if is_cjk(ch) || is_emoji_like(ch) {
        return 2;
    }
    2
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x11FF
            | 0x2E80..=0x2EFF
            | 0x2F00..=0x2FDF
            | 0x2FF0..=0x2FFF
            | 0x3000..=0x303F
            | 0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0x3100..=0x312F
            | 0x3130..=0x318F
            | 0x31A0..=0x31BF
            | 0x31C0..=0x31EF
            | 0x31F0..=0x31FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0x20000..=0x2FA1F
    )
}

fn is_emoji_like(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1F300..=0x1F5FF
            | 0x1F600..=0x1F64F
            | 0x1F680..=0x1F6FF
            | 0x1F700..=0x1F77F
            | 0x1F780..=0x1F7FF
            | 0x1F800..=0x1F8FF
            | 0x1F900..=0x1F9FF
            | 0x1FA00..=0x1FAFF
            | 0x2600..=0x26FF
            | 0x2700..=0x27BF
    )
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

fn load_linkedin_auth() -> Result<LinkedinAuth, AppError> {
    let access_token = env::var("LINKEDIN_ACCESS_TOKEN").map_err(|_| AppError::MissingAuth {
        message: "No LinkedIn access token found.".to_string(),
        suggestion: Some(
            "Set LINKEDIN_ACCESS_TOKEN in .env after completing OAuth authorization.".to_string(),
        ),
        command: Some("outbox auth linkedin guide".to_string()),
    })?;

    let author_urn = env::var("LINKEDIN_AUTHOR_URN").map_err(|_| AppError::MissingAuth {
        message: "No LinkedIn author URN found.".to_string(),
        suggestion: Some(
            "Set LINKEDIN_AUTHOR_URN (example: urn:li:person:...) in .env.".to_string(),
        ),
        command: Some("outbox auth linkedin guide".to_string()),
    })?;

    let version =
        env::var("LINKEDIN_API_VERSION").unwrap_or_else(|_| DEFAULT_LINKEDIN_VERSION.to_string());
    let refresh_token = env_non_empty("LINKEDIN_REFRESH_TOKEN");

    Ok(LinkedinAuth {
        access_token,
        refresh_token,
        author_urn,
        version,
    })
}

fn load_x_auth() -> Result<XAuth, AppError> {
    let access_token = env_non_empty("X_ACCESS_TOKEN").ok_or(AppError::MissingAuth {
        message: "No X access token found.".to_string(),
        suggestion: Some("Set X_ACCESS_TOKEN in .env from your X app user authorization.".to_string()),
        command: Some("outbox publish x --file <path>".to_string()),
    })?;
    Ok(XAuth { access_token })
}

fn env_non_empty(key: &str) -> Option<String> {
    let value = env::var(key).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn parse_post_input(file_path: &PathBuf, config: &RuntimeConfig) -> Result<ParsedPostInput, AppError> {
    let content = fs::read_to_string(file_path).map_err(|err| AppError::Io {
        message: format!("Failed to read content file: {err}"),
    })?;

    let media_refs = extract_obsidian_embeds(&content);
    let media_paths = resolve_media_paths(file_path, &media_refs, config)?;

    let publish_section = extract_publish_section_after_last_separator(&content);
    let publish_text_without_embeds = remove_obsidian_embed_placeholders(&publish_section);
    let publish_text = trim_outer_empty_lines(&publish_text_without_embeds);
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

fn extract_publish_section_after_last_separator(raw: &str) -> String {
    let mut last_sep_end = None;
    let mut cursor = 0usize;
    for line in raw.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim();
        if trimmed == "---" {
            last_sep_end = Some(cursor + line.len());
        }
        cursor += line.len();
    }
    match last_sep_end {
        Some(idx) => raw[idx..].to_string(),
        None => raw.to_string(),
    }
}

fn trim_outer_empty_lines(raw: &str) -> String {
    raw.trim().to_string()
}

fn extract_obsidian_embeds(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while let Some(open_rel) = raw[start..].find("![[") {
        let open = start + open_rel + 3;
        if let Some(close_rel) = raw[open..].find("]]") {
            let close = open + close_rel;
            let inner = raw[open..close].trim();
            if !inner.is_empty() {
                let cleaned = inner
                    .split('|')
                    .next()
                    .unwrap_or(inner)
                    .split('#')
                    .next()
                    .unwrap_or(inner)
                    .trim();
                if !cleaned.is_empty() {
                    out.push(cleaned.to_string());
                }
            }
            start = close + 2;
        } else {
            break;
        }
    }
    out
}

fn remove_obsidian_embed_placeholders(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut start = 0usize;
    while let Some(open_rel) = raw[start..].find("![[") {
        let open = start + open_rel;
        out.push_str(&raw[start..open]);
        let content_start = open + 3;
        if let Some(close_rel) = raw[content_start..].find("]]") {
            let close = content_start + close_rel + 2;
            start = close;
        } else {
            out.push_str(&raw[open..]);
            start = raw.len();
            break;
        }
    }
    if start < raw.len() {
        out.push_str(&raw[start..]);
    }
    out
}

fn resolve_media_paths(
    note_path: &PathBuf,
    refs: &[String],
    config: &RuntimeConfig,
) -> Result<Vec<PathBuf>, AppError> {
    let mut resolved = Vec::new();
    let note_dir = note_path.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));

    for media_ref in refs {
        let ref_path = PathBuf::from(media_ref);
        let ext = ref_path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "png" && ext != "jpg" && ext != "jpeg" {
            return Err(AppError::Validation {
                message: format!(
                    "Unsupported media extension for '{}'. Allowed: .png, .jpg, .jpeg",
                    media_ref
                ),
                suggestion: Some("Convert image to allowed extension and retry.".to_string()),
                command: None,
            });
        }

        let mut candidates: Vec<PathBuf> = Vec::new();
        if ref_path.is_absolute() {
            candidates.push(ref_path.clone());
        } else {
            candidates.push(note_dir.join(&ref_path));
            for base in &config.media_lookup_paths {
                candidates.push(base.join(&ref_path));
            }
        }

        let found = candidates.into_iter().find(|p| p.is_file());
        let Some(path) = found else {
            return Err(AppError::Validation {
                message: format!("Referenced media file not found: '{}'", media_ref),
                suggestion: Some(
                    "Place media in note folder or configure [media].lookup_paths in config.toml."
                        .to_string(),
                ),
                command: None,
            });
        };

        let canon = fs::canonicalize(&path).unwrap_or(path.clone());
        resolved.push(canon);
    }

    Ok(resolved)
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

    for required in ["tweet.read", "tweet.write", "users.read"] {
        if !parts.iter().any(|p| p == required) {
            parts.push(required.to_string());
        }
    }

    if !parts.iter().any(|p| p == "offline.access") {
        parts.push("offline.access".to_string());
    }

    parts.join(" ")
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
    let Ok(raw) = fs::read_to_string(PUBLISH_LOG_PATH) else {
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
    fs::create_dir_all(".outbox").map_err(|err| AppError::Io {
        message: format!("Failed to create .outbox directory: {err}"),
    })?;
    let line = serde_json::to_string(entry).map_err(|err| AppError::Io {
        message: format!("Failed to encode publish log entry: {err}"),
    })?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(PUBLISH_LOG_PATH)
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
        command: Some("outbox auth linkedin guide".to_string()),
    })?;
    let client_secret = env::var("LINKEDIN_CLIENT_SECRET").map_err(|_| AppError::MissingAuth {
        message: "LINKEDIN_CLIENT_SECRET is missing.".to_string(),
        suggestion: Some("Set LINKEDIN_CLIENT_SECRET in .env from your LinkedIn app settings.".to_string()),
        command: Some("outbox auth linkedin guide".to_string()),
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
    fs::create_dir_all(".outbox").map_err(|err| AppError::Io {
        message: format!("Failed to create .outbox directory: {err}"),
    })?;
    let raw = serde_json::to_string(state).map_err(|err| AppError::Io {
        message: format!("Failed to encode OAuth state: {err}"),
    })?;
    fs::write(OAUTH_STATE_PATH, raw).map_err(|err| AppError::Io {
        message: format!("Failed to write OAuth state file: {err}"),
    })
}

fn validate_oauth_state(input_state: &str) -> Result<(), AppError> {
    let raw = fs::read_to_string(OAUTH_STATE_PATH).map_err(|_| AppError::Validation {
        message: "OAuth state file not found. Start login again.".to_string(),
        suggestion: Some("Run outbox auth linkedin login, then retry exchange with returned state.".to_string()),
        command: Some("outbox auth linkedin login".to_string()),
    })?;
    let expected: OAuthState = serde_json::from_str(&raw).map_err(|err| AppError::Io {
        message: format!("Failed to parse OAuth state file: {err}"),
    })?;
    if input_state != expected.state {
        return Err(AppError::Validation {
            message: "OAuth state mismatch.".to_string(),
            suggestion: Some("Use the exact state value returned by login command.".to_string()),
            command: Some("outbox auth linkedin login".to_string()),
        });
    }
    Ok(())
}

fn clear_oauth_state_file() {
    let _ = fs::remove_file(OAUTH_STATE_PATH);
}

fn upsert_env_value(key: &str, value: &str) -> Result<(), AppError> {
    let mut lines = if let Ok(raw) = fs::read_to_string(ENV_PATH) {
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

    fs::write(ENV_PATH, output).map_err(|err| AppError::Io {
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

fn print_json<T: Serialize>(value: &T, pretty: bool) {
    if pretty {
        if let Ok(text) = serde_json::to_string_pretty(value) {
            println!("{text}");
            return;
        }
    }

    if let Ok(text) = serde_json::to_string(value) {
        println!("{text}");
        return;
    }

    println!(
        "{{\"ok\":false,\"error_type\":\"serialization_error\",\"message\":\"Failed to render JSON output.\"}}"
    );
}
