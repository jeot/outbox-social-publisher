use std::env;

use url::Url;

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

#[derive(Debug, Clone)]
pub(crate) struct SubstackAuth {
    pub(crate) session_token: String,
    pub(crate) publication_url: Url,
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

pub(crate) fn load_substack_auth() -> Result<SubstackAuth, AppError> {
    let session_token = env_non_empty("SUBSTACK_SESSION_TOKEN").ok_or(
        AppError::MissingAuth {
            message: "No Substack session token found.".to_string(),
            suggestion: Some(
                "Copy the value of the substack.sid browser cookie into SUBSTACK_SESSION_TOKEN in the workspace .env file."
                    .to_string(),
            ),
            command: Some("publo auth substack guide".to_string()),
        },
    )?;
    if session_token.starts_with("substack.sid=")
        || session_token.contains(';')
        || session_token.contains('\n')
        || session_token.contains('\r')
    {
        return Err(AppError::Validation {
            message: "SUBSTACK_SESSION_TOKEN must contain only the substack.sid cookie value."
                .to_string(),
            suggestion: Some(
                "Remove the 'substack.sid=' prefix and do not paste a complete Cookie header."
                    .to_string(),
            ),
            command: Some("publo auth substack guide".to_string()),
        });
    }

    let publication_url_raw = env_non_empty("SUBSTACK_PUBLICATION_URL").ok_or(
        AppError::MissingAuth {
            message: "No Substack publication URL found.".to_string(),
            suggestion: Some(
                "Set SUBSTACK_PUBLICATION_URL to an HTTPS publication URL such as https://yourname.substack.com."
                    .to_string(),
            ),
            command: Some("publo auth substack guide".to_string()),
        },
    )?;
    let mut publication_url = Url::parse(&publication_url_raw).map_err(|err| {
        AppError::Validation {
            message: format!("Invalid SUBSTACK_PUBLICATION_URL: {err}"),
            suggestion: Some("Use a complete HTTPS publication URL.".to_string()),
            command: Some("publo auth substack guide".to_string()),
        }
    })?;
    if publication_url.scheme() != "https"
        || publication_url.host_str().is_none()
        || !publication_url.username().is_empty()
        || publication_url.password().is_some()
    {
        return Err(AppError::Validation {
            message: "SUBSTACK_PUBLICATION_URL must be an HTTPS URL without embedded credentials."
                .to_string(),
            suggestion: Some("Use a URL such as https://yourname.substack.com.".to_string()),
            command: Some("publo auth substack guide".to_string()),
        });
    }
    publication_url.set_path("/");
    publication_url.set_query(None);
    publication_url.set_fragment(None);

    Ok(SubstackAuth {
        session_token,
        publication_url,
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
