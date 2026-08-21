use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Text;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::publish;

#[derive(Clone)]
struct ApiState {
    catalog_roots: Arc<Vec<PathBuf>>,
    media_lookup_paths: Arc<Vec<PathBuf>>,
    db_path: Arc<PathBuf>,
}

#[derive(Debug, Serialize)]
struct CatalogNode {
    name: String,
    path: String,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<CatalogNode>>,
}

#[derive(Debug, Serialize)]
struct CatalogRootResult {
    root: String,
    ok: bool,
    tree: Vec<CatalogNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileQuery {
    path: String,
}

#[derive(Debug, Deserialize)]
struct ReadyPathPayload {
    path: String,
}

#[derive(Debug, Serialize)]
struct ReadyFileState {
    path: String,
    operator: Option<String>,
}

#[derive(Debug, QueryableByName)]
struct ReadyFileStateRow {
    #[diesel(sql_type = Text)]
    file_path: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    operator: Option<String>,
}

#[derive(Debug, QueryableByName)]
struct ReadyJobRow {
    #[diesel(sql_type = Text)]
    id: String,
}

#[derive(Debug, QueryableByName)]
struct FileJobRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    batch_id: String,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    platform: Option<String>,
    #[diesel(sql_type = Text)]
    workspace_id: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    owner_user_id: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    operator: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    user_note: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    ai_note: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    ai_model: Option<String>,
    #[diesel(sql_type = Text)]
    tags: String,
    #[diesel(sql_type = Text)]
    file_path: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    run_at_utc: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    timezone: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    status_reason: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    attempt_count: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    file_sha256: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    text_sha256: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    fingerprint: Option<String>,
    #[diesel(sql_type = Text)]
    created_at: String,
    #[diesel(sql_type = Text)]
    updated_at: String,
}

pub async fn run_server(
    host: String,
    port: u16,
    catalog_roots: Vec<PathBuf>,
    media_lookup_paths: Vec<PathBuf>,
    db_path: PathBuf,
    pretty_json: bool,
) -> ExitCode {
    let state = ApiState {
        catalog_roots: Arc::new(catalog_roots),
        media_lookup_paths: Arc::new(media_lookup_paths),
        db_path: Arc::new(db_path),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/catalog/tree", get(catalog_tree))
        .route("/api/catalog/file", get(catalog_file))
        .route("/api/catalog/preview", get(catalog_preview))
        .route("/api/catalog/media", get(catalog_media))
        .route("/api/jobs/ready/mark", post(ready_mark))
        .route("/api/jobs/ready/unmark", post(ready_unmark))
        .with_state(state);

    let bind_addr = format!("{host}:{port}");
    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => listener,
        Err(err) => {
            let value = json!({
                "ok": false,
                "error_type": "bind_error",
                "message": format!("Failed to bind API server on {bind_addr}: {err}")
            });
            print_json(&value, pretty_json);
            return ExitCode::from(1);
        }
    };

    println!("Publo API listening on http://{bind_addr}");
    println!("Health: http://{bind_addr}/health");

    if let Err(err) = axum::serve(listener, app).await {
        let value = json!({
            "ok": false,
            "error_type": "server_error",
            "message": format!("API server error: {err}")
        });
        print_json(&value, pretty_json);
        return ExitCode::from(1);
    }

    ExitCode::from(0)
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn catalog_tree(State(state): State<ApiState>) -> Json<Value> {
    let roots: Vec<CatalogRootResult> = state
        .catalog_roots
        .iter()
        .map(|root| {
            let root_display = root.to_string_lossy().to_string();
            match build_tree(root, 0) {
                Ok(tree) => CatalogRootResult {
                    root: root_display,
                    ok: true,
                    tree,
                    error: None,
                },
                Err(err) => CatalogRootResult {
                    root: root_display,
                    ok: false,
                    tree: Vec::new(),
                    error: Some(err),
                },
            }
        })
        .collect();
    let ready_states = list_ready_states(state.db_path.as_path()).unwrap_or_default();
    Json(json!({ "ok": true, "roots": roots, "ready_states": ready_states }))
}

async fn catalog_file(State(state): State<ApiState>, Query(query): Query<FileQuery>) -> Json<Value> {
    let requested = PathBuf::from(&query.path);
    if !is_valid_markdown_request(&requested, state.catalog_roots.as_slice()) {
        return Json(json!({
            "ok": false,
            "message": "Path must be an absolute .md file inside configured catalog roots."
        }));
    }

    match fs::read_to_string(&requested) {
        Ok(content) => {
            let jobs = list_jobs_for_path(state.db_path.as_path(), &requested).unwrap_or_default();
            let ready_job = jobs
                .iter()
                .find(|job| job.get("status").and_then(|v| v.as_str()) == Some("ready"));
            Json(json!({
                "ok": true,
                "path": canonical_or_original(&requested),
                "content": content,
                "jobs": jobs,
                "ready": {
                    "is_ready": ready_job.is_some(),
                    "job_id": ready_job.and_then(|job| job.get("id")).and_then(|v| v.as_str()),
                    "operator": ready_job.and_then(|job| job.get("operator")).and_then(|v| v.as_str())
                }
            }))
        }
        Err(err) => Json(json!({
            "ok": false,
            "message": format!("Failed to read file: {err}")
        })),
    }
}

async fn catalog_preview(State(state): State<ApiState>, Query(query): Query<FileQuery>) -> Json<Value> {
    let requested = PathBuf::from(&query.path);
    if !is_valid_markdown_request(&requested, state.catalog_roots.as_slice()) {
        return Json(json!({
            "ok": false,
            "message": "Path must be an absolute .md file inside configured catalog roots."
        }));
    }

    let content = match fs::read_to_string(&requested) {
        Ok(content) => content,
        Err(err) => {
            return Json(json!({
                "ok": false,
                "message": format!("Failed to read file: {err}")
            }));
        }
    };

    let publish_text = publish::extract_publish_text(&content);
    let media_refs = publish::extract_obsidian_embeds(&content);
    let media = publish::collect_media_preview(&requested, &media_refs, state.media_lookup_paths.as_slice());
    let issues = publish::preview_issues(&publish_text, &media);

    Json(json!({
        "ok": true,
        "path": requested.to_string_lossy(),
        "preview": {
            "publish_text": publish_text,
            "media_refs": media_refs,
            "media": media,
            "issues": issues,
            "publishable": issues.is_empty()
        }
    }))
}

async fn catalog_media(
    State(state): State<ApiState>,
    Query(query): Query<FileQuery>,
) -> Response {
    let requested = PathBuf::from(&query.path);
    let allowed_roots: Vec<PathBuf> = state
        .catalog_roots
        .iter()
        .chain(state.media_lookup_paths.iter())
        .cloned()
        .collect();
    if !is_valid_media_request(&requested, allowed_roots.as_slice()) {
        return (StatusCode::BAD_REQUEST, "Invalid media path").into_response();
    }

    let bytes = match fs::read(&requested) {
        Ok(bytes) => bytes,
        Err(_) => return (StatusCode::NOT_FOUND, "Media file not found").into_response(),
    };

    let content_type = media_content_type(&requested).unwrap_or("application/octet-stream");
    let mut response = bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type),
    );
    response
}

async fn ready_mark(
    State(state): State<ApiState>,
    Json(payload): Json<ReadyPathPayload>,
) -> Json<Value> {
    let requested = PathBuf::from(&payload.path);
    if !is_valid_markdown_request(&requested, state.catalog_roots.as_slice()) {
        return Json(json!({
            "ok": false,
            "message": "Path must be an absolute .md file inside configured catalog roots."
        }));
    }

    let canonical = match requested.canonicalize() {
        Ok(path) => path,
        Err(err) => {
            return Json(json!({
                "ok": false,
                "message": format!("Failed to resolve file path: {err}")
            }));
        }
    };

    if let Some(job_id) = ready_job_id_for_path(state.db_path.as_path(), &canonical) {
        return Json(json!({
            "ok": true,
            "mode": "ready_mark",
            "created": false,
            "job_id": job_id,
            "path": canonical.to_string_lossy(),
            "ready": true
        }));
    }

    let content = match fs::read_to_string(&canonical) {
        Ok(content) => content,
        Err(err) => {
            return Json(json!({
                "ok": false,
                "message": format!("Failed to read file: {err}")
            }));
        }
    };

    let file_sha256 = sha256_hex(content.as_bytes());
    let text_sha256 = sha256_hex(publish::extract_publish_text(&content).as_bytes());
    let id = generate_id();
    let batch_id = generate_id();
    let now = Utc::now().to_rfc3339();

    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => {
            return Json(json!({ "ok": false, "message": message }));
        }
    };

    let insert_result = sql_query(
        "INSERT INTO jobs (
            id, batch_id, kind, status, file_path, file_sha256, text_sha256, created_at, updated_at
         ) VALUES (
            ?, ?, 'catalog', 'ready', ?, ?, ?, ?, ?
         )",
    )
    .bind::<Text, _>(&id)
    .bind::<Text, _>(&batch_id)
    .bind::<Text, _>(canonical.to_string_lossy().to_string())
    .bind::<Text, _>(&file_sha256)
    .bind::<Text, _>(&text_sha256)
    .bind::<Text, _>(&now)
    .bind::<Text, _>(&now)
    .execute(&mut conn);

    match insert_result {
        Ok(_) => Json(json!({
            "ok": true,
            "mode": "ready_mark",
            "created": true,
            "job_id": id,
            "path": canonical.to_string_lossy(),
            "ready": true
        })),
        Err(err) => Json(json!({
            "ok": false,
            "message": format!("Failed to mark file as ready: {err}")
        })),
    }
}

async fn ready_unmark(
    State(state): State<ApiState>,
    Json(payload): Json<ReadyPathPayload>,
) -> Json<Value> {
    let requested = PathBuf::from(&payload.path);
    if !is_valid_markdown_request(&requested, state.catalog_roots.as_slice()) {
        return Json(json!({
            "ok": false,
            "message": "Path must be an absolute .md file inside configured catalog roots."
        }));
    }
    let canonical = requested.canonicalize().unwrap_or(requested.clone());

    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => {
            return Json(json!({ "ok": false, "message": message }));
        }
    };

    let canonical_str = canonical.to_string_lossy().to_string();
    let requested_str = requested.to_string_lossy().to_string();

    let mut removed = sql_query("DELETE FROM jobs WHERE status = 'ready' AND file_path = ?")
        .bind::<Text, _>(&canonical_str)
        .execute(&mut conn)
        .unwrap_or(0);

    if requested_str != canonical_str {
        removed += sql_query("DELETE FROM jobs WHERE status = 'ready' AND file_path = ?")
            .bind::<Text, _>(&requested_str)
            .execute(&mut conn)
            .unwrap_or(0);
    }

    Json(json!({
        "ok": true,
        "mode": "ready_unmark",
        "path": canonical_str,
        "ready": false,
        "removed": removed
    }))
}

fn build_tree(root: &Path, depth: usize) -> Result<Vec<CatalogNode>, String> {
    if depth > 8 {
        return Ok(Vec::new());
    }
    let read_dir = fs::read_dir(root).map_err(|err| format!("Failed to read directory: {err}"))?;
    let mut entries: Vec<fs::DirEntry> = read_dir.filter_map(Result::ok).collect();
    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    let mut nodes = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            let children = build_tree(&path, depth + 1)?;
            nodes.push(CatalogNode {
                name,
                path: canonical_or_original(&path),
                kind: "dir",
                children: Some(children),
            });
            continue;
        }
        if file_type.is_file() && name.to_ascii_lowercase().ends_with(".md") {
            nodes.push(CatalogNode {
                name,
                path: canonical_or_original(&path),
                kind: "file",
                children: None,
            });
        }
    }
    Ok(nodes)
}

fn is_path_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    let canonical_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    roots.iter().any(|root| {
        root.canonicalize()
            .map(|canonical_root| canonical_path.starts_with(canonical_root))
            .unwrap_or(false)
    })
}

fn is_valid_markdown_request(path: &Path, roots: &[PathBuf]) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if extension != "md" {
        return false;
    }
    is_path_within_roots(path, roots)
}

fn is_valid_media_request(path: &Path, roots: &[PathBuf]) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
        return false;
    }
    is_path_within_roots(path, roots)
}

fn media_content_type(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        _ => None,
    }
}

fn canonical_or_original(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn generate_id() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 16] = rng.random();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn open_db_by_path(path: &Path) -> Result<SqliteConnection, String> {
    let db_url = path.to_string_lossy().into_owned();
    SqliteConnection::establish(&db_url)
        .map_err(|err| format!("Failed to open SQLite DB at {}: {err}", path.display()))
}

fn list_ready_states(db_path: &Path) -> Result<Vec<ReadyFileState>, String> {
    let mut conn = open_db_by_path(db_path)?;
    let rows: Vec<ReadyFileStateRow> = sql_query(
        "SELECT j.file_path, j.operator
         FROM jobs j
         JOIN (
            SELECT file_path, MAX(created_at) AS max_created_at
            FROM jobs
            WHERE status = 'ready'
            GROUP BY file_path
         ) latest
         ON latest.file_path = j.file_path
         AND latest.max_created_at = j.created_at
         WHERE j.status = 'ready'
         ORDER BY j.file_path ASC",
    )
    .load(&mut conn)
    .map_err(|err| format!("Failed to load ready states: {err}"))?;

    Ok(rows
        .into_iter()
        .map(|row| ReadyFileState {
            path: row.file_path,
            operator: row.operator,
        })
        .collect())
}

fn ready_job_id_for_path(db_path: &Path, path: &Path) -> Option<String> {
    let mut conn = open_db_by_path(db_path).ok()?;
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_str = canonical.to_string_lossy().to_string();
    let mut rows: Vec<ReadyJobRow> = sql_query(
        "SELECT id
         FROM jobs
         WHERE status = 'ready' AND file_path = ?
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind::<Text, _>(&canonical_str)
    .load(&mut conn)
    .ok()?;

    if let Some(row) = rows.pop() {
        return Some(row.id);
    }

    let requested_str = path.to_string_lossy().to_string();
    if requested_str == canonical_str {
        return None;
    }

    let mut rows_fallback: Vec<ReadyJobRow> = sql_query(
        "SELECT id
         FROM jobs
         WHERE status = 'ready' AND file_path = ?
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind::<Text, _>(&requested_str)
    .load(&mut conn)
    .ok()?;

    rows_fallback.pop().map(|row| row.id)
}

fn list_jobs_for_path(db_path: &Path, path: &Path) -> Result<Vec<Value>, String> {
    let mut conn = open_db_by_path(db_path)?;
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_str = canonical.to_string_lossy().to_string();
    let requested_str = path.to_string_lossy().to_string();

    let rows: Vec<FileJobRow> = sql_query(
        "SELECT id,batch_id,kind,status,platform,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
         FROM jobs
         WHERE file_path = ? OR file_path = ?
         ORDER BY updated_at DESC, created_at DESC",
    )
    .bind::<Text, _>(&canonical_str)
    .bind::<Text, _>(&requested_str)
    .load(&mut conn)
    .map_err(|err| format!("Failed to load jobs for file: {err}"))?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let tags: Value = serde_json::from_str(&row.tags).unwrap_or_else(|_| json!([]));
            json!({
                "id": row.id,
                "batch_id": row.batch_id,
                "kind": row.kind,
                "status": row.status,
                "platform": row.platform,
                "workspace_id": row.workspace_id,
                "owner_user_id": row.owner_user_id,
                "operator": row.operator,
                "user_note": row.user_note,
                "ai_note": row.ai_note,
                "ai_model": row.ai_model,
                "tags": tags,
                "file_path": row.file_path,
                "run_at_utc": row.run_at_utc,
                "timezone": row.timezone,
                "status_reason": row.status_reason,
                "attempt_count": row.attempt_count,
                "file_sha256": row.file_sha256,
                "text_sha256": row.text_sha256,
                "fingerprint": row.fingerprint,
                "created_at": row.created_at,
                "updated_at": row.updated_at
            })
        })
        .collect())
}

fn print_json(value: &Value, pretty: bool) {
    if pretty {
        if let Ok(serialized) = serde_json::to_string_pretty(value) {
            println!("{serialized}");
            return;
        }
    }
    println!("{}", value);
}
