use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use url::Url;

use crate::errors::AppError;

#[derive(Debug, Serialize, Clone)]
pub(crate) struct MediaPreviewItem {
    pub(crate) reference: String,
    pub(crate) resolved_path: Option<String>,
    pub(crate) exists: bool,
    pub(crate) valid_extension: bool,
    pub(crate) error: Option<String>,
}

pub(crate) fn extract_publish_text(raw: &str) -> String {
    let publish_section = extract_publish_section_after_last_separator(raw);
    let publish_text_without_embeds = remove_obsidian_embed_placeholders(&publish_section);
    trim_outer_empty_lines(&publish_text_without_embeds)
}

pub(crate) fn preview_issues(publish_text: &str, media: &[MediaPreviewItem]) -> Vec<String> {
    let mut issues: Vec<String> = Vec::new();
    if publish_text.trim().is_empty() {
        issues.push("publish_text_empty".to_string());
    }
    if media.iter().any(|item| !item.valid_extension) {
        issues.push("unsupported_media_extension".to_string());
    }
    if media.iter().any(|item| !item.exists) {
        issues.push("missing_media_file".to_string());
    }
    issues
}

pub(crate) fn validate_linkedin_media_count(media_count: usize) -> Result<(), AppError> {
    if media_count > 20 {
        return Err(AppError::Validation {
            message: format!(
                "LinkedIn multi-image supports at most 20 images; found {}.",
                media_count
            ),
            suggestion: Some("Reduce image count to 20 or fewer and retry.".to_string()),
            command: None,
        });
    }
    Ok(())
}

pub(crate) fn validate_x_media_count(media_count: usize) -> Result<(), AppError> {
    if media_count > 4 {
        return Err(AppError::Validation {
            message: format!("X supports at most 4 images per post; found {}.", media_count),
            suggestion: Some("Reduce image count to 4 or fewer and retry.".to_string()),
            command: None,
        });
    }
    Ok(())
}

pub(crate) fn validate_x_post_text(
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
            command: Some("publo publish x --file <path>".to_string()),
        });
    }

    let weighted_len = x_weighted_length(text);
    if !allow_length && weighted_len > 280 {
        return Err(AppError::Validation {
            message: format!("X post is too long by weighted count: {} > 280.", weighted_len),
            suggestion: Some(
                "Shorten text (URLs count as 23 chars; many non-ASCII/emoji chars count as 2), or use --allow-length to bypass local check."
                    .to_string(),
            ),
            command: Some("publo publish x --file <path>".to_string()),
        });
    }

    Ok(())
}

pub(crate) fn extract_cashtags(text: &str) -> Vec<String> {
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

pub(crate) fn x_weighted_length(text: &str) -> usize {
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

pub(crate) fn extract_publish_section_after_last_separator(raw: &str) -> String {
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

pub(crate) fn trim_outer_empty_lines(raw: &str) -> String {
    raw.trim().to_string()
}

pub(crate) fn extract_obsidian_embeds(raw: &str) -> Vec<String> {
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

pub(crate) fn remove_obsidian_embed_placeholders(raw: &str) -> String {
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

pub(crate) fn collect_media_preview(
    note_path: &Path,
    refs: &[String],
    media_lookup_paths: &[PathBuf],
) -> Vec<MediaPreviewItem> {
    refs.iter()
        .map(|media_ref| {
            let ref_path = PathBuf::from(media_ref);
            let valid_extension = has_allowed_media_extension(&ref_path);
            if !valid_extension {
                return MediaPreviewItem {
                    reference: media_ref.clone(),
                    resolved_path: None,
                    exists: false,
                    valid_extension: false,
                    error: Some("unsupported extension (allowed: .png, .jpg, .jpeg)".to_string()),
                };
            }

            let found = resolve_existing_media_path(note_path, &ref_path, media_lookup_paths);
            match found {
                Some(path) => {
                    let canon = fs::canonicalize(&path).unwrap_or(path);
                    MediaPreviewItem {
                        reference: media_ref.clone(),
                        resolved_path: Some(canon.to_string_lossy().to_string()),
                        exists: true,
                        valid_extension: true,
                        error: None,
                    }
                }
                None => MediaPreviewItem {
                    reference: media_ref.clone(),
                    resolved_path: None,
                    exists: false,
                    valid_extension: true,
                    error: Some(
                        "file not found (note folder first, then [media].lookup_paths)".to_string(),
                    ),
                },
            }
        })
        .collect()
}

pub(crate) fn resolve_media_paths(
    note_path: &Path,
    refs: &[String],
    media_lookup_paths: &[PathBuf],
) -> Result<Vec<PathBuf>, AppError> {
    let mut resolved = Vec::new();

    for media_ref in refs {
        let ref_path = PathBuf::from(media_ref);
        if !has_allowed_media_extension(&ref_path) {
            return Err(AppError::Validation {
                message: format!(
                    "Unsupported media extension for '{}'. Allowed: .png, .jpg, .jpeg",
                    media_ref
                ),
                suggestion: Some("Convert image to allowed extension and retry.".to_string()),
                command: None,
            });
        }

        let found = resolve_existing_media_path(note_path, &ref_path, media_lookup_paths);
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

        let canon = fs::canonicalize(&path).unwrap_or(path);
        resolved.push(canon);
    }

    Ok(resolved)
}

fn resolve_existing_media_path(
    note_path: &Path,
    ref_path: &Path,
    media_lookup_paths: &[PathBuf],
) -> Option<PathBuf> {
    let note_dir = note_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut candidates: Vec<PathBuf> = Vec::new();
    if ref_path.is_absolute() {
        candidates.push(ref_path.to_path_buf());
    } else {
        candidates.push(note_dir.join(ref_path));
        for base in media_lookup_paths {
            candidates.push(base.join(ref_path));
        }
    }

    candidates.into_iter().find(|p| p.is_file())
}

fn has_allowed_media_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ext == "png" || ext == "jpg" || ext == "jpeg"
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
