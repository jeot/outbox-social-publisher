use std::path::Path;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::header::{ACCEPT, CONTENT_TYPE, COOKIE, HeaderMap, ORIGIN, REFERER, USER_AGENT};
use reqwest::{Method, StatusCode, Url, redirect};
use serde_json::{Value, json};

use crate::auth::SubstackAuth;
use crate::errors::AppError;

const SUBSTACK_API_BASE: &str = "https://substack.com/api/v1/";
const SUBSTACK_ORIGIN: &str = "https://substack.com";
const SUBSTACK_NOTES_REFERER: &str = "https://substack.com/notes";
const SUBSTACK_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
const MAX_NOTE_LENGTH_UTF16: usize = 5_000;

#[derive(Debug, Clone)]
pub(crate) struct SubstackProfile {
    pub(crate) id: Option<String>,
    pub(crate) handle: String,
    pub(crate) name: Option<String>,
}

#[derive(Debug)]
pub(crate) struct SubstackPublishReceipt {
    pub(crate) id: Option<String>,
    pub(crate) request_id: Option<String>,
}

#[derive(Debug)]
struct SubstackResponse {
    body: Value,
    request_id: Option<String>,
}

pub(crate) struct SubstackClient {
    http: reqwest::Client,
    api_base: Url,
    session_cookie: String,
}

impl SubstackClient {
    pub(crate) fn new(
        auth: &SubstackAuth,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .redirect(redirect::Policy::none())
            .build()
            .map_err(|err| AppError::Http {
                message: format!("Failed to build Substack HTTP client: {err}"),
                status: None,
                api_error: None,
                retryable: false,
            })?;
        let api_base = Url::parse(SUBSTACK_API_BASE).expect("valid Substack API base URL");
        Ok(Self {
            http,
            api_base,
            session_cookie: format!("substack.sid={}", auth.session_token),
        })
    }

    #[cfg(test)]
    fn with_api_base(auth: &SubstackAuth, api_base: Url) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .redirect(redirect::Policy::none())
            .build()
            .map_err(|err| AppError::Http {
                message: format!("Failed to build Substack test HTTP client: {err}"),
                status: None,
                api_error: None,
                retryable: false,
            })?;
        Ok(Self {
            http,
            api_base,
            session_cookie: format!("substack.sid={}", auth.session_token),
        })
    }

    pub(crate) async fn get_authenticated_profile(&self) -> Result<SubstackProfile, AppError> {
        let options = self
            .request(Method::GET, "handle/options", None, false)
            .await?;
        let handle = options
            .body
            .get("potentialHandles")
            .and_then(Value::as_array)
            .and_then(|handles| {
                handles.iter().find_map(|item| {
                    let is_existing = item.get("type").and_then(Value::as_str) == Some("existing");
                    is_existing
                        .then(|| item.get("handle").and_then(Value::as_str))
                        .flatten()
                })
            })
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| AppError::Http {
                message: "Authenticated Substack profile was not found.".to_string(),
                status: Some(502),
                api_error: Some(options.body),
                retryable: false,
            })?;

        let profile_path = format!("user/{}/public_profile", urlencoding::encode(&handle));
        let profile = self
            .request(Method::GET, &profile_path, None, false)
            .await?;
        Ok(SubstackProfile {
            id: value_as_string(profile.body.get("id")),
            handle,
            name: profile
                .body
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    pub(crate) async fn upload_image(&self, path: &Path) -> Result<Value, AppError> {
        let bytes = std::fs::read(path).map_err(|err| AppError::Io {
            message: format!("Failed to read Substack image '{}': {err}", path.display()),
        })?;
        let mime = image_mime(path)?;
        let data_url = format!("data:{mime};base64,{}", STANDARD.encode(bytes));
        let response = self
            .request(
                Method::POST,
                "image",
                Some(json!({ "image": data_url })),
                false,
            )
            .await?;
        let url = response
            .body
            .get("url")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::Http {
                message: "Substack image upload response did not include a URL.".to_string(),
                status: Some(502),
                api_error: Some(response.body.clone()),
                retryable: false,
            })?;
        Url::parse(url).map_err(|err| AppError::Http {
            message: format!("Substack image upload returned an invalid URL: {err}"),
            status: Some(502),
            api_error: Some(response.body.clone()),
            retryable: false,
        })?;
        Ok(response.body)
    }

    pub(crate) async fn create_image_attachment(
        &self,
        uploaded_image: &Value,
    ) -> Result<String, AppError> {
        let image_url = uploaded_image
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation {
                message: "Uploaded Substack image has no URL.".to_string(),
                suggestion: None,
                command: None,
            })?;
        let response = self
            .request(
                Method::POST,
                "comment/attachment/",
                Some(json!({ "url": image_url, "type": "image" })),
                false,
            )
            .await?;
        extract_id(&response.body).ok_or_else(|| AppError::Http {
            message: "Substack image attachment response did not include an ID.".to_string(),
            status: Some(502),
            api_error: Some(response.body),
            retryable: false,
        })
    }

    pub(crate) async fn publish_note(
        &self,
        body_json: Value,
        attachment_ids: Vec<String>,
    ) -> Result<SubstackPublishReceipt, AppError> {
        let mut payload = json!({
            "bodyJson": body_json,
            "tabId": "for-you",
            "surface": "feed",
            "replyMinimumRole": "everyone"
        });
        if !attachment_ids.is_empty() {
            payload
                .as_object_mut()
                .expect("Substack Note payload is an object")
                .insert("attachmentIds".to_string(), json!(attachment_ids));
        }
        let response = self
            .request(Method::POST, "comment/feed/", Some(payload), true)
            .await?;
        Ok(SubstackPublishReceipt {
            id: extract_id(&response.body),
            request_id: response.request_id,
        })
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        final_publish: bool,
    ) -> Result<SubstackResponse, AppError> {
        let url = self
            .api_base
            .join(path)
            .map_err(|err| AppError::Validation {
                message: format!("Failed to build Substack API URL: {err}"),
                suggestion: None,
                command: None,
            })?;
        let mut request = self
            .http
            .request(method, url.clone())
            .header(ACCEPT, "application/json")
            .header(COOKIE, &self.session_cookie)
            .header(USER_AGENT, SUBSTACK_BROWSER_USER_AGENT)
            .header(ORIGIN, SUBSTACK_ORIGIN)
            .header(REFERER, SUBSTACK_NOTES_REFERER);
        if let Some(body) = body {
            request = request.header(CONTENT_TYPE, "application/json").json(&body);
        }
        let response = request.send().await.map_err(|err| {
            let local_hint = if final_publish {
                "substack_publish_outcome_unknown"
            } else {
                "substack_transport_error"
            };
            AppError::Http {
                message: if final_publish {
                    format!(
                        "Substack Note publish response was not received; publishing outcome is unknown: {err}"
                    )
                } else {
                    format!("Substack request failed: {err}")
                },
                status: None,
                api_error: Some(json!({ "local_hint": local_hint })),
                retryable: !final_publish && (err.is_timeout() || err.is_connect()),
            }
        })?;
        let status = response.status();
        let request_id = response_request_id(response.headers());
        let raw = response.text().await.map_err(|err| AppError::Http {
            message: if final_publish {
                format!(
                    "Substack Note publish response could not be read; publishing outcome is unknown: {err}"
                )
            } else {
                format!("Failed to read Substack response: {err}")
            },
            status: Some(status.as_u16()),
            api_error: final_publish.then(|| {
                json!({ "local_hint": "substack_publish_outcome_unknown" })
            }),
            retryable: false,
        })?;
        if !status.is_success() {
            let local_hint = if status == StatusCode::UNAUTHORIZED {
                Some("substack_session_invalid")
            } else if status == StatusCode::FORBIDDEN && final_publish {
                Some("substack_publish_forbidden")
            } else {
                None
            };
            let provider_error = serde_json::from_str::<Value>(&raw)
                .unwrap_or_else(|_| json!({ "response_preview": truncate(&raw, 500) }));
            return Err(AppError::Http {
                message: format!("Substack API returned {}", status.as_u16()),
                status: Some(status.as_u16()),
                api_error: Some(json!({
                    "local_hint": local_hint,
                    "provider_error": provider_error
                })),
                retryable: !final_publish
                    && (status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS),
            });
        }
        let parsed = if raw.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str::<Value>(&raw).map_err(|_| AppError::Http {
                message: if final_publish {
                    "Substack accepted the publish request but returned a non-JSON response; publishing outcome is unknown."
                        .to_string()
                } else {
                    "Substack returned a non-JSON response.".to_string()
                },
                status: Some(status.as_u16()),
                api_error: Some(json!({
                    "local_hint": final_publish
                        .then_some("substack_publish_outcome_unknown"),
                    "response_preview": truncate(&raw, 500)
                })),
                retryable: false,
            })?
        };
        Ok(SubstackResponse {
            body: parsed,
            request_id,
        })
    }
}

pub(crate) fn build_note_body(text: &str) -> Result<Value, AppError> {
    let utf16_len = text.encode_utf16().count();
    if text.is_empty() || utf16_len > MAX_NOTE_LENGTH_UTF16 {
        return Err(AppError::Validation {
            message: format!(
                "Substack Note body must contain 1 to {MAX_NOTE_LENGTH_UTF16} UTF-16 code units; found {utf16_len}."
            ),
            suggestion: Some("Shorten the Note and retry.".to_string()),
            command: None,
        });
    }
    let content = text
        .split('\n')
        .map(|line| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            json!({
                "type": "paragraph",
                "content": if line.is_empty() {
                    Vec::<Value>::new()
                } else {
                    vec![json!({ "type": "text", "text": line })]
                }
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "type": "doc",
        "attrs": { "schemaVersion": "v1", "title": null },
        "content": content
    }))
}

pub(crate) fn note_payload_preview(body_json: Value, media_count: usize) -> Value {
    let attachment_ids = (0..media_count)
        .map(|index| format!("<resolved-via-substack-image-attachment-{}>", index + 1))
        .collect::<Vec<_>>();
    let mut payload = json!({
        "bodyJson": body_json,
        "tabId": "for-you",
        "surface": "feed",
        "replyMinimumRole": "everyone"
    });
    if !attachment_ids.is_empty() {
        payload
            .as_object_mut()
            .expect("Substack Note preview payload is an object")
            .insert("attachmentIds".to_string(), json!(attachment_ids));
    }
    payload
}

fn image_mime(path: &Path) -> Result<&'static str, AppError> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok("image/png"),
        Some("jpg") | Some("jpeg") => Ok("image/jpeg"),
        _ => Err(AppError::Validation {
            message: format!("Unsupported Substack image format: {}", path.display()),
            suggestion: Some("Use PNG, JPG, or JPEG images.".to_string()),
            command: None,
        }),
    }
}

fn extract_id(value: &Value) -> Option<String> {
    value_as_string(value.get("id"))
        .or_else(|| value_as_string(value.pointer("/comment/id")))
        .or_else(|| value_as_string(value.pointer("/item/comment/id")))
}

fn value_as_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn response_request_id(headers: &HeaderMap) -> Option<String> {
    ["x-request-id", "cf-ray", "x-amzn-trace-id"]
        .iter()
        .find_map(|name| {
            headers
                .get(*name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        })
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Json;
    use axum::Router;
    use axum::extract::State;
    use axum::http::HeaderMap as AxumHeaderMap;
    use axum::http::StatusCode as AxumStatusCode;
    use axum::routing::{get, post};
    use tokio::sync::Mutex;

    use super::*;

    type RecordedCall = (String, Value, Option<String>);

    #[derive(Clone, Default)]
    struct TestState {
        calls: Arc<Mutex<Vec<RecordedCall>>>,
    }

    #[test]
    fn builds_multiline_note_body_like_pinned_sdk() {
        let body = build_note_body("First line\n\nThird line").expect("build body");
        assert_eq!(body["attrs"]["schemaVersion"], "v1");
        assert_eq!(body["attrs"]["title"], Value::Null);
        assert_eq!(body["content"].as_array().map(Vec::len), Some(3));
        assert_eq!(body["content"][1]["content"], json!([]));
    }

    #[test]
    fn enforces_pinned_sdk_note_length() {
        let too_long = "a".repeat(MAX_NOTE_LENGTH_UTF16 + 1);
        assert!(build_note_body(&too_long).is_err());
        assert!(build_note_body("").is_err());
    }

    #[tokio::test]
    async fn reproduces_profile_image_attachment_and_publish_calls() {
        let state = TestState::default();
        let app = Router::new()
            .route("/api/v1/handle/options", get(handle_options))
            .route("/api/v1/user/writer/public_profile", get(public_profile))
            .route("/api/v1/image", post(upload_image))
            .route("/api/v1/comment/attachment/", post(create_attachment))
            .route("/api/v1/comment/feed/", post(publish_note))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });

        let auth = SubstackAuth {
            session_token: "session-value".to_string(),
            publication_url: Url::parse("https://writer.substack.com").unwrap(),
        };
        let client = SubstackClient::with_api_base(
            &auth,
            Url::parse(&format!("http://{address}/api/v1/")).unwrap(),
        )
        .expect("build test client");
        let profile = client
            .get_authenticated_profile()
            .await
            .expect("load profile");
        assert_eq!(profile.handle, "writer");

        let text_receipt = client
            .publish_note(build_note_body("Text only").unwrap(), vec![])
            .await
            .expect("publish text-only note");
        assert_eq!(text_receipt.id.as_deref(), Some("303"));

        let image_path =
            std::env::temp_dir().join(format!("publo-substack-test-{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&image_path, b"image bytes").expect("write test image");
        let uploaded = client
            .upload_image(&image_path)
            .await
            .expect("upload image");
        let attachment_id = client
            .create_image_attachment(&uploaded)
            .await
            .expect("create attachment");
        let receipt = client
            .publish_note(
                build_note_body("Hello from Publo").unwrap(),
                vec![attachment_id],
            )
            .await
            .expect("publish note");
        std::fs::remove_file(image_path).expect("remove test image");
        assert_eq!(receipt.id.as_deref(), Some("303"));

        let calls = state.calls.lock().await;
        assert_eq!(calls.len(), 6);
        assert!(
            calls
                .iter()
                .all(|call| { call.2.as_deref() == Some("substack.sid=session-value") })
        );
        assert_eq!(calls[2].0, "/api/v1/comment/feed/");
        assert!(calls[2].1.get("attachmentIds").is_none());
        assert_eq!(calls[3].0, "/api/v1/image");
        assert_eq!(
            calls[4].1,
            json!({ "url": "https://cdn.example/image.png", "type": "image" })
        );
        assert_eq!(calls[5].1["attachmentIds"], json!(["attachment-1"]));
    }

    #[tokio::test]
    async fn reports_missing_images_before_an_api_request() {
        let auth = SubstackAuth {
            session_token: "session-value".to_string(),
            publication_url: Url::parse("https://writer.substack.com").unwrap(),
        };
        let client =
            SubstackClient::with_api_base(&auth, Url::parse("http://127.0.0.1:1/api/v1/").unwrap())
                .expect("build test client");
        let missing = std::env::temp_dir().join(format!(
            "publo-missing-substack-image-{}.png",
            uuid::Uuid::new_v4()
        ));
        assert!(matches!(
            client.upload_image(&missing).await,
            Err(AppError::Io { .. })
        ));
    }

    #[tokio::test]
    async fn marks_unauthorized_sessions_with_a_recovery_hint() {
        let app = Router::new().route(
            "/api/v1/handle/options",
            get(|| async {
                (
                    AxumStatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "invalid session" })),
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind auth test server");
        let address = listener.local_addr().expect("auth test server address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve auth test app");
        });
        let auth = SubstackAuth {
            session_token: "expired".to_string(),
            publication_url: Url::parse("https://writer.substack.com").unwrap(),
        };
        let client = SubstackClient::with_api_base(
            &auth,
            Url::parse(&format!("http://{address}/api/v1/")).unwrap(),
        )
        .expect("build auth test client");
        let error = client
            .get_authenticated_profile()
            .await
            .expect_err("expired session should fail");
        let output = error.to_output();
        assert_eq!(output.http_status, Some(401));
        assert_eq!(
            output
                .api_error
                .as_ref()
                .and_then(|value| value.get("local_hint"))
                .and_then(Value::as_str),
            Some("substack_session_invalid")
        );
    }

    #[tokio::test]
    async fn reports_forbidden_publish_as_rejected_not_unknown() {
        let app = Router::new().route(
            "/api/v1/comment/feed/",
            post(|| async {
                (
                    AxumStatusCode::FORBIDDEN,
                    "<!DOCTYPE html><title>Error</title>",
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind forbidden test server");
        let address = listener
            .local_addr()
            .expect("forbidden test server address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve forbidden test app");
        });
        let auth = SubstackAuth {
            session_token: "session-value".to_string(),
            publication_url: Url::parse("https://writer.substack.com").unwrap(),
        };
        let client = SubstackClient::with_api_base(
            &auth,
            Url::parse(&format!("http://{address}/api/v1/")).unwrap(),
        )
        .expect("build forbidden test client");
        let error = client
            .publish_note(build_note_body("Rejected").unwrap(), vec![])
            .await
            .expect_err("forbidden publish should fail");
        let output = error.to_output();
        assert_eq!(output.http_status, Some(403));
        assert_eq!(
            output
                .api_error
                .as_ref()
                .and_then(|value| value.get("local_hint"))
                .and_then(Value::as_str),
            Some("substack_publish_forbidden")
        );
        assert!(!output.message.contains("unknown"));
    }

    async fn record(state: TestState, headers: AxumHeaderMap, path: &str, body: Value) {
        assert_eq!(
            headers
                .get("user-agent")
                .and_then(|value| value.to_str().ok()),
            Some(SUBSTACK_BROWSER_USER_AGENT)
        );
        assert_eq!(
            headers.get("origin").and_then(|value| value.to_str().ok()),
            Some(SUBSTACK_ORIGIN)
        );
        assert_eq!(
            headers.get("referer").and_then(|value| value.to_str().ok()),
            Some(SUBSTACK_NOTES_REFERER)
        );
        let cookie = headers
            .get("cookie")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        state
            .calls
            .lock()
            .await
            .push((path.to_string(), body, cookie));
    }

    async fn handle_options(State(state): State<TestState>, headers: AxumHeaderMap) -> Json<Value> {
        record(state, headers, "/api/v1/handle/options", Value::Null).await;
        Json(json!({ "potentialHandles": [{ "handle": "writer", "type": "existing" }] }))
    }

    async fn public_profile(State(state): State<TestState>, headers: AxumHeaderMap) -> Json<Value> {
        record(
            state,
            headers,
            "/api/v1/user/writer/public_profile",
            Value::Null,
        )
        .await;
        Json(json!({ "id": 7, "handle": "writer", "name": "Writer" }))
    }

    async fn upload_image(
        State(state): State<TestState>,
        headers: AxumHeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        record(state, headers, "/api/v1/image", body).await;
        Json(json!({ "id": 1, "url": "https://cdn.example/image.png" }))
    }

    async fn create_attachment(
        State(state): State<TestState>,
        headers: AxumHeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        record(state, headers, "/api/v1/comment/attachment/", body).await;
        Json(json!({ "id": "attachment-1" }))
    }

    async fn publish_note(
        State(state): State<TestState>,
        headers: AxumHeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        record(state, headers, "/api/v1/comment/feed/", body).await;
        Json(json!({ "id": 303 }))
    }
}
