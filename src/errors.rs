use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
pub(crate) struct ErrorOutput {
    pub(crate) ok: bool,
    pub(crate) error_type: &'static str,
    pub(crate) message: String,
    pub(crate) http_status: Option<u16>,
    pub(crate) api_error: Option<Value>,
    pub(crate) retryable: bool,
    pub(crate) suggestion: Option<String>,
    pub(crate) command: Option<String>,
}

#[derive(Debug)]
pub(crate) enum AppError {
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

impl From<diesel::result::Error> for AppError {
    fn from(error: diesel::result::Error) -> Self {
        AppError::Io {
            message: error.to_string(),
        }
    }
}

impl AppError {
    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            AppError::Validation { .. } => 2,
            AppError::MissingAuth { .. } => 3,
            AppError::Io { .. } => 4,
            AppError::Http { .. } => 5,
            AppError::DuplicatePublish { .. } => 6,
        }
    }

    pub(crate) fn to_output(&self) -> ErrorOutput {
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

    if let Some(402) = status
        && let Some(err) = api_error
    {
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
