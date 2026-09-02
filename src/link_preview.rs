use reqwest::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use scraper::{Html, Selector};
use serde::Serialize;
use url::Url;

use crate::errors::AppError;

const MAX_METADATA_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct LinkPreviewState {
    pub(crate) status: &'static str,
    pub(crate) url: Option<String>,
    pub(crate) domain: Option<String>,
    pub(crate) reason: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct LinkMetadata {
    pub(crate) url: String,
    pub(crate) domain: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) image_url: Option<String>,
}

pub(crate) fn detect_first_link(text: &str) -> Option<Url> {
    let mut offsets = text
        .match_indices("https://")
        .chain(text.match_indices("http://"))
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    offsets.sort_unstable();

    for offset in offsets {
        let tail = &text[offset..];
        let end = tail
            .find(|character: char| character.is_whitespace() || matches!(character, '<' | '>' | '"' | '\''))
            .unwrap_or(tail.len());
        let raw = tail[..end].trim_end_matches(|character: char| {
            matches!(character, '.' | ',' | '!' | '?' | ';' | ':' | ')' | ']' | '}')
        });
        if let Ok(url) = Url::parse(raw)
            && matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
        {
            return Some(url);
        }
    }

    None
}

pub(crate) fn preview_state(text: &str, suppressed_by_media: bool) -> LinkPreviewState {
    let Some(url) = detect_first_link(text) else {
        return LinkPreviewState {
            status: "not_found",
            url: None,
            domain: None,
            reason: None,
        };
    };
    let domain = url.host_str().map(str::to_string);
    if suppressed_by_media {
        return LinkPreviewState {
            status: "suppressed_by_media",
            url: Some(url.to_string()),
            domain,
            reason: Some("linkedin_native_media_takes_precedence"),
        };
    }
    LinkPreviewState {
        status: "found",
        url: Some(url.to_string()),
        domain,
        reason: None,
    }
}

pub(crate) async fn fetch_metadata(
    client: &reqwest::Client,
    url: &Url,
) -> Result<LinkMetadata, AppError> {
    let response = client
        .get(url.clone())
        .header(ACCEPT, "text/html,application/xhtml+xml")
        .header(USER_AGENT, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36")
        .send()
        .await
        .map_err(|err| AppError::Http {
            message: format!("Failed to load link-preview metadata from {url}: {err}"),
            status: None,
            api_error: None,
            retryable: err.is_timeout() || err.is_connect(),
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::Http {
            message: format!("Link-preview page returned {} for {url}", status.as_u16()),
            status: Some(status.as_u16()),
            api_error: None,
            retryable: status.is_server_error() || status.as_u16() == 429,
        });
    }
    if let Some(content_type) = response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok())
        && !content_type.to_ascii_lowercase().contains("text/html")
        && !content_type.to_ascii_lowercase().contains("application/xhtml+xml")
    {
        return Err(AppError::Validation {
            message: format!("Link-preview URL is not an HTML page: {url}"),
            suggestion: Some("Use a public article page with title metadata.".to_string()),
            command: None,
        });
    }
    let final_url = response.url().clone();
    let bytes = response.bytes().await.map_err(|err| AppError::Http {
        message: format!("Failed to read link-preview metadata from {url}: {err}"),
        status: Some(status.as_u16()),
        api_error: None,
        retryable: false,
    })?;
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(AppError::Validation {
            message: format!("Link-preview HTML exceeds the {} byte limit.", MAX_METADATA_BYTES),
            suggestion: Some("Use a smaller public article page.".to_string()),
            command: None,
        });
    }
    let html = String::from_utf8_lossy(&bytes);
    Ok(parse_metadata(&final_url, &html))
}

pub(crate) fn parse_metadata(url: &Url, html: &str) -> LinkMetadata {
    let document = Html::parse_document(html);
    let title = meta_content(&document, "meta[property='og:title']")
        .or_else(|| meta_content(&document, "meta[name='twitter:title']"))
        .or_else(|| element_text(&document, "title"))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| url.host_str().unwrap_or("Link").to_string());
    let description = meta_content(&document, "meta[property='og:description']")
        .or_else(|| meta_content(&document, "meta[name='twitter:description']"))
        .or_else(|| meta_content(&document, "meta[name='description']"));
    let image_url = meta_content(&document, "meta[property='og:image']")
        .or_else(|| meta_content(&document, "meta[name='twitter:image']"))
        .and_then(|value| url.join(&value).ok())
        .map(|value| value.to_string());

    LinkMetadata {
        url: url.to_string(),
        domain: url.host_str().unwrap_or_default().to_string(),
        title: truncate_chars(&title, 399),
        description: description.map(|value| truncate_chars(&value, 4_085)),
        image_url,
    }
}

fn meta_content(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .find_map(|element| element.value().attr("content"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn element_text(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .next()
        .map(|element| element.text().collect::<String>())
        .map(|value| value.trim().to_string())
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_first_http_link_and_trims_markdown_punctuation() {
        let found = detect_first_link(
            "Read [this article](https://writer.substack.com/p/example). Then https://example.com",
        )
        .expect("detect URL");
        assert_eq!(found.as_str(), "https://writer.substack.com/p/example");
    }

    #[test]
    fn reports_link_suppression_for_linkedin_media() {
        let state = preview_state("See https://example.com/story", true);
        assert_eq!(state.status, "suppressed_by_media");
        assert_eq!(state.reason, Some("linkedin_native_media_takes_precedence"));
    }

    #[test]
    fn parses_open_graph_metadata_and_relative_image() {
        let url = Url::parse("https://example.com/articles/one").unwrap();
        let metadata = parse_metadata(
            &url,
            r#"<html><head>
                <title>Fallback</title>
                <meta property="og:title" content="Article title">
                <meta property="og:description" content="Article description">
                <meta property="og:image" content="/cover.png">
            </head></html>"#,
        );
        assert_eq!(metadata.title, "Article title");
        assert_eq!(metadata.description.as_deref(), Some("Article description"));
        assert_eq!(metadata.image_url.as_deref(), Some("https://example.com/cover.png"));
    }
}
