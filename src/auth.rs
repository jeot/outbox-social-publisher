use std::env;

use crate::errors::AppError;

const DEFAULT_LINKEDIN_VERSION: &str = "202601";

#[derive(Debug)]
pub(crate) struct LinkedinAuth {
    pub(crate) access_token: String,
    pub(crate) refresh_token: Option<String>,
    pub(crate) author_urn: String,
    pub(crate) version: String,
}

#[derive(Debug)]
pub(crate) struct XAuth {
    pub(crate) access_token: String,
    pub(crate) refresh_token: Option<String>,
}

pub(crate) fn load_linkedin_auth() -> Result<LinkedinAuth, AppError> {
    let access_token = env::var("LINKEDIN_ACCESS_TOKEN").map_err(|_| AppError::MissingAuth {
        message: "No LinkedIn access token found.".to_string(),
        suggestion: Some(
            "Set LINKEDIN_ACCESS_TOKEN in .env after completing OAuth authorization.".to_string(),
        ),
        command: Some("publo auth linkedin guide".to_string()),
    })?;
    let author_urn = env::var("LINKEDIN_AUTHOR_URN").map_err(|_| AppError::MissingAuth {
        message: "No LinkedIn author URN found.".to_string(),
        suggestion: Some("Set LINKEDIN_AUTHOR_URN (example: urn:li:person:...) in .env.".to_string()),
        command: Some("publo auth linkedin guide".to_string()),
    })?;
    let version =
        env_non_empty("LINKEDIN_API_VERSION").unwrap_or_else(|| DEFAULT_LINKEDIN_VERSION.to_string());
    let refresh_token = env_non_empty("LINKEDIN_REFRESH_TOKEN");
    Ok(LinkedinAuth {
        access_token,
        refresh_token,
        author_urn,
        version,
    })
}

pub(crate) fn load_x_auth() -> Result<XAuth, AppError> {
    let access_token = env_non_empty("X_ACCESS_TOKEN").ok_or(AppError::MissingAuth {
        message: "No X access token found.".to_string(),
        suggestion: Some("Set X_ACCESS_TOKEN in .env from your X app user authorization.".to_string()),
        command: Some("publo publish x --file <path>".to_string()),
    })?;
    Ok(XAuth {
        access_token,
        refresh_token: env_non_empty("X_REFRESH_TOKEN"),
    })
}

pub(crate) fn env_non_empty(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|raw| {
        let value = raw.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}
