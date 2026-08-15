use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use rand::Rng;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use urlencoding::encode;

const DEFAULT_LINKEDIN_VERSION: &str = "202601";
const OAUTH_STATE_PATH: &str = ".outbox/linkedin_oauth_state";
const ENV_PATH: &str = ".env";

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
}

#[derive(Debug, Args)]
struct PublishLinkedinArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Args)]
struct AuthArgs {
    #[command(subcommand)]
    platform: AuthPlatform,
}

#[derive(Debug, Subcommand)]
enum AuthPlatform {
    Linkedin(AuthLinkedinArgs),
}

#[derive(Debug, Args)]
struct AuthLinkedinArgs {
    #[command(subcommand)]
    command: AuthLinkedinCommand,
}

#[derive(Debug, Subcommand)]
enum AuthLinkedinCommand {
    Guide,
    Login(AuthLinkedinLoginArgs),
    Exchange(AuthLinkedinExchangeArgs),
    Whoami,
}

#[derive(Debug, Args)]
struct AuthLinkedinLoginArgs {
    #[arg(long, default_value_t = false)]
    open_browser: bool,
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

#[derive(Debug)]
struct RuntimeConfig {
    pretty_json: bool,
    connect_timeout: Duration,
    request_timeout: Duration,
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
}

impl AppError {
    fn exit_code(&self) -> u8 {
        match self {
            AppError::Validation { .. } => 2,
            AppError::MissingAuth { .. } => 3,
            AppError::Io { .. } => 4,
            AppError::Http { .. } => 5,
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
                suggestion: Some("Inspect api_error for provider details and retry when resolved.".to_string()),
                command: None,
            },
        }
    }
}

#[derive(Debug)]
struct LinkedinAuth {
    access_token: String,
    author_urn: String,
    version: String,
}

#[tokio::main]
async fn main() -> ExitCode {
    let _ = dotenvy::from_filename_override(".env");
    let config = load_config();
    let cli = Cli::parse();

    let result: Result<Value, AppError> = match cli.command {
        Commands::Publish(publish) => match publish.platform {
            PublishPlatform::Linkedin(args) => publish_linkedin(args, &config).await,
        },
        Commands::Auth(auth) => match auth.platform {
            AuthPlatform::Linkedin(args) => match args.command {
                AuthLinkedinCommand::Guide => show_linkedin_auth_guide(),
                AuthLinkedinCommand::Login(args) => start_linkedin_login(args),
                AuthLinkedinCommand::Exchange(args) => exchange_linkedin_code(args, &config).await,
                AuthLinkedinCommand::Whoami => resolve_linkedin_author_urn(&config).await,
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

    RuntimeConfig {
        pretty_json,
        connect_timeout,
        request_timeout,
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

fn start_linkedin_login(args: AuthLinkedinLoginArgs) -> Result<Value, AppError> {
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

    let browser_opened = if args.open_browser {
        webbrowser::open(&auth_url).ok().map(|_| true)
    } else {
        None
    };

    Ok(json!({
        "ok": true,
        "platform": "linkedin",
        "mode": "auth_login",
        "auth_url": auth_url,
        "state": state,
        "browser_opened": browser_opened,
        "next_command": "outbox auth linkedin exchange --code <code-from-redirect-url> --state <state-from-login>",
        "note": "Open auth_url in a browser, approve access, then copy the code query parameter from redirect URL."
    }))
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
        ("grant_type", "authorization_code".to_string()),
        ("code", args.code.clone()),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
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

    let body = response
        .json::<TokenExchangeResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse token exchange response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })?;

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

    let response = client
        .get("https://api.linkedin.com/v2/userinfo")
        .header(AUTHORIZATION, format!("Bearer {access_token}"))
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

    let userinfo = response
        .json::<UserInfoResponse>()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to parse userinfo response: {err}"),
            status: Some(200),
            api_error: None,
            retryable: false,
        })?;

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

async fn publish_linkedin(args: PublishLinkedinArgs, config: &RuntimeConfig) -> Result<Value, AppError> {
    if !args.file.exists() {
        return Err(AppError::Validation {
            message: format!("Content file does not exist: {}", args.file.display()),
            suggestion: Some("Check the file path and run the same command again.".to_string()),
            command: None,
        });
    }

    let content = fs::read_to_string(&args.file).map_err(|err| AppError::Io {
        message: format!("Failed to read content file: {err}"),
    })?;

    let commentary = content.trim().to_string();
    if commentary.is_empty() {
        return Err(AppError::Validation {
            message: "Content file is empty after trimming whitespace.".to_string(),
            suggestion: Some("Add post text to the file.".to_string()),
            command: None,
        });
    }

    let auth = load_linkedin_auth()?;
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

    let payload = json!({
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
        .post("https://api.linkedin.com/rest/posts")
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("LinkedIn request failed: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;

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

    let output = SuccessOutput {
        ok: true,
        platform: "linkedin",
        post_id,
        post_url: None,
        request_id,
        published_at: Utc::now().to_rfc3339(),
    };

    serde_json::to_value(output).map_err(|err| AppError::Io {
        message: format!("Failed to serialize success output: {err}"),
    })
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

    Ok(LinkedinAuth {
        access_token,
        author_urn,
        version,
    })
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
                *line = format!("{key}={value}");
                found = true;
                break;
            }
        }
    }

    if !found {
        lines.push(format!("{key}={value}"));
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
