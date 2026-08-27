use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use axum::extract::{Path as RoutePath, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text};
use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::cli::{
    JobCancelArgs, JobIdArgs, JobScheduleArgs, JobUnscheduleArgs, OperatorArg, PlatformArg,
};
use crate::config::RuntimeConfig;
use crate::publish;
use crate::{
    job_cancel, job_preflight_for_file_platform, job_schedule, job_unready, job_unschedule,
};

#[derive(Clone)]
struct ApiState {
    catalog_roots: Arc<Vec<PathBuf>>,
    media_lookup_paths: Arc<Vec<PathBuf>>,
    db_path: Arc<PathBuf>,
    runtime_config: Arc<RuntimeConfig>,
    web_dist_path: Arc<Option<PathBuf>>,
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

#[derive(Debug, Deserialize)]
struct JobIdPayload {
    id: String,
}

#[derive(Debug, Deserialize)]
struct JobPlatformSetPayload {
    id: String,
    platform: String,
}

#[derive(Debug, Deserialize)]
struct JobSchedulePayload {
    id: String,
    at: String,
    timezone: Option<String>,
    platform: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobScheduleMultiPayload {
    id: String,
    at: String,
    timezone: Option<String>,
    platforms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct JobReadyPlatformsPayload {
    id: String,
    platforms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct JobUnschedulePayload {
    id: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobCancelPayload {
    id: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobTimePayload {
    id: String,
    at: String,
    timezone: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReadyFileState {
    path: String,
    operator: Option<String>,
}

#[derive(Debug, Serialize)]
struct CatalogJobState {
    path: String,
    badges: Vec<CatalogJobBadge>,
}

#[derive(Debug, Serialize)]
struct CatalogJobBadge {
    status: String,
    platform: Option<String>,
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
struct CatalogJobStateRow {
    #[diesel(sql_type = Text)]
    file_path: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    platform: Option<String>,
}

#[derive(Debug, QueryableByName)]
struct RevivableJobRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Text)]
    asset_id: String,
}

#[derive(Debug, QueryableByName)]
struct FileJobRow {
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
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    platform: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    publish_mode: Option<String>,
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
    selected_platforms: String,
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

#[derive(Debug, QueryableByName)]
struct ReadySourceJobRow {
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Text)]
    content_group_id: String,
    #[diesel(sql_type = Text)]
    asset_id: String,
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
    selected_platforms: String,
    #[diesel(sql_type = Text)]
    file_path: String,
}

pub async fn run_server(host: String, port: u16, config: RuntimeConfig) -> ExitCode {
    let web_dist_path = resolve_web_dist_path();
    let pretty_json = config.pretty_json;
    let state = ApiState {
        catalog_roots: Arc::new(config.catalog_roots.clone()),
        media_lookup_paths: Arc::new(config.media_lookup_paths.clone()),
        db_path: Arc::new(config.db_path.clone()),
        runtime_config: Arc::new(config),
        web_dist_path: Arc::new(web_dist_path),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/catalog/tree", get(catalog_tree))
        .route("/api/catalog/file", get(catalog_file))
        .route("/api/catalog/preview", get(catalog_preview))
        .route("/api/catalog/media", get(catalog_media))
        .route("/api/jobs/ready", get(ready_jobs))
        .route("/api/jobs/scheduled", get(scheduled_jobs))
        .route("/api/jobs/blocked", get(blocked_jobs))
        .route("/api/jobs/canceled", get(canceled_jobs))
        .route("/api/jobs/disabled", get(disabled_jobs))
        .route("/api/jobs/ready/mark", post(ready_mark))
        .route("/api/jobs/ready/unmark", post(ready_unmark))
        .route("/api/jobs/unready", post(ready_unready))
        .route("/api/jobs/platforms", post(set_ready_platforms))
        .route("/api/jobs/time", post(set_ready_time))
        .route("/api/jobs/platform/set", post(job_set_platform))
        .route("/api/jobs/platform/clear", post(job_clear_platform))
        .route("/api/jobs/schedule", post(schedule_job))
        .route("/api/jobs/schedule-multi", post(schedule_multi_job))
        .route("/api/jobs/scheduled/time", post(set_scheduled_time))
        .route("/api/jobs/unschedule", post(unschedule_job))
        .route("/api/jobs/cancel", post(cancel_job))
        .route("/", get(spa_index))
        .route("/{*path}", get(spa_asset))
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

fn resolve_web_dist_path() -> Option<PathBuf> {
    let cwd_candidate = std::env::current_dir().ok()?.join("web").join("dist");
    if cwd_candidate.is_dir() {
        return Some(cwd_candidate);
    }

    let exe_parent = std::env::current_exe().ok()?;
    let exe_parent = exe_parent.parent()?;
    let exe_candidate = exe_parent.join("web").join("dist");
    if exe_candidate.is_dir() {
        return Some(exe_candidate);
    }

    None
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
    let job_states = list_catalog_job_states(state.db_path.as_path()).unwrap_or_default();
    Json(json!({ "ok": true, "roots": roots, "ready_states": ready_states, "job_states": job_states }))
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
    let action_group_id = generate_id();
    let content_group_id = generate_id();
    let canonical_str = canonical.to_string_lossy().to_string();
    let requested_str = requested.to_string_lossy().to_string();
    let asset_id = asset_id_for_paths(
        state.db_path.as_path(),
        canonical_str.as_str(),
        requested_str.as_str(),
    )
    .unwrap_or_else(generate_id);
    let now = Utc::now().to_rfc3339();

    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => {
            return Json(json!({ "ok": false, "message": message }));
        }
    };

    if let Some(job_id) = ready_job_id_for_path(state.db_path.as_path(), &canonical) {
        let purged = purge_terminal_jobs_for_path(
            &mut conn,
            &canonical_str,
            &requested_str,
            Some(job_id.as_str()),
        );
        return Json(json!({
            "ok": true,
            "mode": "ready_mark",
            "created": false,
            "job_id": job_id,
            "path": canonical_str,
            "ready": true,
            "purged": purged
        }));
    }

    if let Some(job_id) = ready_job_id_for_asset(state.db_path.as_path(), &asset_id) {
        let refreshed_action_group_id = generate_id();
        let update_result = sql_query(
            "UPDATE jobs
             SET status = 'ready',
                 platform = NULL,
                 publish_mode = NULL,
                 run_at_utc = NULL,
                 timezone = NULL,
                 status_reason = NULL,
                 action_group_id = ?,
                 file_path = ?,
                 file_sha256 = ?,
                 text_sha256 = ?,
                 updated_at = ?
             WHERE id = ?",
        )
        .bind::<Text, _>(&refreshed_action_group_id)
        .bind::<Text, _>(&canonical_str)
        .bind::<Text, _>(&file_sha256)
        .bind::<Text, _>(&text_sha256)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&job_id)
        .execute(&mut conn);
        return match update_result {
            Ok(affected) if affected > 0 => Json(json!({
                "ok": true,
                "mode": "ready_mark",
                "created": false,
                "job_id": job_id,
                "path": canonical_str,
                "ready": true
            })),
            Ok(_) => Json(json!({
                "ok": false,
                "message": "No job updated while refreshing ready state."
            })),
            Err(err) => Json(json!({
                "ok": false,
                "message": format!("Failed to refresh existing ready job: {err}")
            })),
        };
    }

    if let Some(row) = latest_revivable_job_for_path(
        state.db_path.as_path(),
        &canonical_str,
        &requested_str,
    ) {
        let revived_action_group_id = generate_id();
        let update_result = sql_query(
            "UPDATE jobs
             SET status = 'ready',
                 platform = NULL,
                 publish_mode = NULL,
                 run_at_utc = NULL,
                 timezone = NULL,
                 status_reason = NULL,
                 action_group_id = ?,
                 file_path = ?,
                 file_sha256 = ?,
                 text_sha256 = ?,
                 updated_at = ?
             WHERE id = ?",
        )
        .bind::<Text, _>(&revived_action_group_id)
        .bind::<Text, _>(&canonical_str)
        .bind::<Text, _>(&file_sha256)
        .bind::<Text, _>(&text_sha256)
        .bind::<Text, _>(&now)
        .bind::<Text, _>(&row.id)
        .execute(&mut conn);

        return match update_result {
            Ok(affected) if affected > 0 => {
                let purged = purge_terminal_jobs_for_path(
                    &mut conn,
                    &canonical_str,
                    &requested_str,
                    Some(row.id.as_str()),
                );
                Json(json!({
                    "ok": true,
                    "mode": "ready_mark",
                    "created": false,
                    "revived": true,
                    "from_status": row.status,
                    "job_id": row.id,
                    "path": canonical_str,
                    "ready": true,
                    "purged": purged
                }))
            }
            Ok(_) => Json(json!({
                "ok": false,
                "message": "No job updated while reviving ready state."
            })),
            Err(err) => Json(json!({
                "ok": false,
                "message": format!("Failed to revive existing job as ready: {err}")
            })),
        };
    }

    let insert_result = sql_query(
        "INSERT INTO jobs (
            id, action_group_id, content_group_id, asset_id, kind, status, workspace_id, file_path, file_sha256, text_sha256, created_at, updated_at
         ) VALUES (
            ?, ?, ?, ?, 'catalog', 'ready', ?, ?, ?, ?, ?, ?
         )",
    )
    .bind::<Text, _>(&id)
    .bind::<Text, _>(&action_group_id)
    .bind::<Text, _>(&content_group_id)
    .bind::<Text, _>(&asset_id)
    .bind::<Text, _>(&state.runtime_config.workspace_id)
    .bind::<Text, _>(&canonical_str)
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
            "path": canonical_str,
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

async fn ready_jobs(State(state): State<ApiState>) -> Json<Value> {
    match list_jobs_by_status(state.db_path.as_path(), "ready") {
        Ok(items) => Json(json!({ "ok": true, "items": items })),
        Err(message) => Json(json!({ "ok": false, "message": message, "items": [] })),
    }
}

async fn scheduled_jobs(State(state): State<ApiState>) -> Json<Value> {
    match list_jobs_by_status(state.db_path.as_path(), "scheduled") {
        Ok(items) => Json(json!({ "ok": true, "items": items })),
        Err(message) => Json(json!({ "ok": false, "message": message, "items": [] })),
    }
}

async fn blocked_jobs(State(state): State<ApiState>) -> Json<Value> {
    match list_jobs_by_status(state.db_path.as_path(), "blocked") {
        Ok(items) => Json(json!({ "ok": true, "items": items })),
        Err(message) => Json(json!({ "ok": false, "message": message, "items": [] })),
    }
}

async fn canceled_jobs(State(state): State<ApiState>) -> Json<Value> {
    match list_jobs_by_status(state.db_path.as_path(), "canceled") {
        Ok(items) => Json(json!({ "ok": true, "items": items })),
        Err(message) => Json(json!({ "ok": false, "message": message, "items": [] })),
    }
}

async fn disabled_jobs(State(state): State<ApiState>) -> Json<Value> {
    match list_jobs_by_status(state.db_path.as_path(), "disabled") {
        Ok(items) => Json(json!({ "ok": true, "items": items })),
        Err(message) => Json(json!({ "ok": false, "message": message, "items": [] })),
    }
}

async fn ready_unready(
    State(state): State<ApiState>,
    Json(payload): Json<JobIdPayload>,
) -> Json<Value> {
    match job_unready(JobIdArgs { id: payload.id }, state.runtime_config.as_ref()) {
        Ok(value) => Json(value),
        Err(err) => Json(json!(err.to_output())),
    }
}

async fn set_ready_platforms(
    State(state): State<ApiState>,
    Json(payload): Json<JobReadyPlatformsPayload>,
) -> Json<Value> {
    let normalized = match normalize_platform_list(payload.platforms.as_slice()) {
        Ok(items) => items,
        Err(message) => {
            return Json(json!({
                "ok": false,
                "error_type": "validation_error",
                "message": message
            }))
        }
    };

    let selected_platforms_json = selected_platforms_json(normalized.as_slice());
    let selected_platforms: Vec<&str> = normalized.iter().map(|platform| platform.as_str()).collect();
    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => return Json(json!({ "ok": false, "message": message })),
    };
    let now = Utc::now().to_rfc3339();
    let changed = sql_query(
        "UPDATE jobs
         SET selected_platforms = ?,
             updated_at = ?,
             version = version + 1,
             synced_at = NULL,
             modified_by = 'local'
         WHERE id = ? AND status IN ('ready', 'blocked', 'canceled', 'disabled')",
    )
    .bind::<Text, _>(&selected_platforms_json)
    .bind::<Text, _>(&now)
    .bind::<Text, _>(&payload.id)
    .execute(&mut conn);

    match changed {
        Ok(0) => Json(json!({
            "ok": false,
            "error_type": "validation_error",
            "message": "Only decision-queue jobs can store selected platforms."
        })),
        Ok(_) => Json(json!({
            "ok": true,
            "job_id": payload.id,
            "selected_platforms": selected_platforms
        })),
        Err(err) => Json(json!({
            "ok": false,
            "error_type": "io_error",
            "message": format!("Failed to update selected platforms: {err}")
        })),
    }
}

async fn job_set_platform(
    State(state): State<ApiState>,
    Json(payload): Json<JobPlatformSetPayload>,
) -> Json<Value> {
    let platform = match parse_platform_arg(payload.platform.as_str()) {
        Some(value) => value,
        None => {
            return Json(json!({
                "ok": false,
                "error_type": "validation_error",
                "message": "Invalid platform. Use linkedin or x."
            }));
        }
    };

    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => return Json(json!({ "ok": false, "message": message })),
    };
    let selected_platforms_json = selected_platforms_json(&[platform]);
    let now = Utc::now().to_rfc3339();
    let changed = sql_query(
        "UPDATE jobs
         SET platform = NULL,
             publish_mode = NULL,
             selected_platforms = ?,
             updated_at = ?,
             version = version + 1,
             synced_at = NULL,
             modified_by = 'local'
         WHERE id = ? AND status IN ('ready', 'blocked', 'canceled', 'disabled')",
    )
    .bind::<Text, _>(&selected_platforms_json)
    .bind::<Text, _>(&now)
    .bind::<Text, _>(&payload.id)
    .execute(&mut conn);

    match changed {
        Ok(0) => Json(json!({
            "ok": false,
            "error_type": "validation_error",
            "message": "Only decision-queue jobs can update selected platforms."
        })),
        Ok(_) => match list_jobs_by_id(state.db_path.as_path(), &payload.id) {
            Ok(Some(job)) => Json(json!({ "ok": true, "job": job })),
            Ok(None) => Json(json!({ "ok": true })),
            Err(message) => Json(json!({ "ok": false, "message": message })),
        },
        Err(err) => Json(json!({
            "ok": false,
            "error_type": "io_error",
            "message": format!("Failed to update selected platforms: {err}")
        })),
    }
}

async fn job_clear_platform(
    State(state): State<ApiState>,
    Json(payload): Json<JobIdPayload>,
) -> Json<Value> {
    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => return Json(json!({ "ok": false, "message": message })),
    };
    let now = Utc::now().to_rfc3339();
    let changed = sql_query(
        "UPDATE jobs
         SET platform = NULL,
             publish_mode = NULL,
             selected_platforms = '[]',
             updated_at = ?,
             version = version + 1,
             synced_at = NULL,
             modified_by = 'local'
         WHERE id = ? AND status IN ('ready', 'blocked', 'canceled', 'disabled')",
    )
    .bind::<Text, _>(&now)
    .bind::<Text, _>(&payload.id)
    .execute(&mut conn);

    match changed {
        Ok(0) => Json(json!({
            "ok": false,
            "error_type": "validation_error",
            "message": "Only decision-queue jobs can clear selected platforms."
        })),
        Ok(_) => match list_jobs_by_id(state.db_path.as_path(), &payload.id) {
            Ok(Some(job)) => Json(json!({ "ok": true, "job": job })),
            Ok(None) => Json(json!({ "ok": true })),
            Err(message) => Json(json!({ "ok": false, "message": message })),
        },
        Err(err) => Json(json!({
            "ok": false,
            "error_type": "io_error",
            "message": format!("Failed to clear platform: {err}")
        })),
    }
}

async fn schedule_job(
    State(state): State<ApiState>,
    Json(payload): Json<JobSchedulePayload>,
) -> Json<Value> {
    let platform = match payload.platform {
        Some(value) => match parse_platform_arg(value.as_str()) {
            Some(parsed) => Some(parsed),
            None => {
                return Json(json!({
                    "ok": false,
                    "error_type": "validation_error",
                    "message": "Invalid platform. Use linkedin or x."
                }));
            }
        },
        None => None,
    };
    let args = JobScheduleArgs {
        id: payload.id,
        platform,
        at: payload.at,
        timezone: payload.timezone,
        by: OperatorArg::User,
        user_note: None,
        ai_note: None,
        ai_model: None,
    };
    match job_schedule(args, state.runtime_config.as_ref()) {
        Ok(value) => Json(value),
        Err(err) => Json(json!(err.to_output())),
    }
}

async fn set_ready_time(
    State(state): State<ApiState>,
    Json(payload): Json<JobTimePayload>,
) -> Json<Value> {
    let run_at_utc = match parse_run_at_to_utc(payload.at.as_str()) {
        Ok(value) => value,
        Err(message) => {
            return Json(json!({
                "ok": false,
                "error_type": "validation_error",
                "message": message
            }))
        }
    };
    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => return Json(json!({ "ok": false, "message": message })),
    };
    let now = Utc::now().to_rfc3339();
    let changed = sql_query(
        "UPDATE jobs
         SET run_at_utc = ?,
             timezone = ?,
             status_reason = NULL,
             updated_at = ?,
             version = version + 1,
             synced_at = NULL,
             modified_by = 'local'
         WHERE id = ? AND status IN ('ready', 'blocked', 'canceled', 'disabled')",
    )
    .bind::<Text, _>(&run_at_utc)
    .bind::<Nullable<Text>, _>(payload.timezone.as_deref())
    .bind::<Text, _>(&now)
    .bind::<Text, _>(&payload.id)
    .execute(&mut conn);

    match changed {
        Ok(0) => Json(json!({
            "ok": false,
            "error_type": "validation_error",
            "message": "Only decision-queue jobs can set schedule time."
        })),
        Ok(_) => match list_jobs_by_id(state.db_path.as_path(), &payload.id) {
            Ok(Some(job)) => Json(json!({ "ok": true, "job": job })),
            Ok(None) => Json(json!({ "ok": true })),
            Err(message) => Json(json!({ "ok": false, "message": message })),
        },
        Err(err) => Json(json!({
            "ok": false,
            "error_type": "io_error",
            "message": format!("Failed to set schedule time: {err}")
        })),
    }
}

async fn schedule_multi_job(
    State(state): State<ApiState>,
    Json(payload): Json<JobScheduleMultiPayload>,
) -> Json<Value> {
    let platforms = match normalize_platform_list(payload.platforms.as_slice()) {
        Ok(items) => items,
        Err(message) => {
            return Json(json!({
                "ok": false,
                "error_type": "validation_error",
                "message": message
            }))
        }
    };

    let source = match load_ready_source_job(state.db_path.as_path(), &payload.id) {
        Ok(Some(job)) => job,
        Ok(None) => {
            return Json(json!({
                "ok": false,
                "error_type": "validation_error",
                "message": format!("Decision job not found: {}", payload.id)
            }))
        }
        Err(message) => return Json(json!({ "ok": false, "message": message })),
    };

    if !matches!(
        source.status.as_str(),
        "ready" | "blocked" | "canceled" | "disabled"
    ) {
        return Json(json!({
            "ok": false,
            "error_type": "validation_error",
            "message": "Only decision-queue jobs can be multi-scheduled."
        }));
    }

    let operator = match source.operator.as_deref() {
        Some("ai") => OperatorArg::Ai,
        _ => OperatorArg::User,
    };
    let common_action_group_id = generate_id();
    let common_content_group_id = if source.content_group_id.trim().is_empty() {
        generate_id()
    } else {
        source.content_group_id.clone()
    };
    let selected_from_decision = selected_platforms_from_json(&source.selected_platforms);
    let platforms = if platforms.is_empty() {
        selected_from_decision
            .iter()
            .filter_map(|raw| parse_platform_arg(raw.as_str()))
            .collect::<Vec<PlatformArg>>()
    } else {
        platforms
    };
    if platforms.is_empty() {
        return Json(json!({
            "ok": false,
            "error_type": "validation_error",
            "message": "Select at least one platform before scheduling."
        }));
    }
    let run_at_utc = match parse_run_at_to_utc(payload.at.as_str()) {
        Ok(value) => value,
        Err(message) => {
            return Json(json!({
                "ok": false,
                "error_type": "validation_error",
                "message": message
            }))
        }
    };

    let mut preflight_ok: Vec<(PlatformArg, crate::SchedulePreflightSnapshot)> = Vec::new();
    let mut preflight_results: Vec<Value> = Vec::new();
    for platform in &platforms {
        match job_preflight_for_file_platform(
            PathBuf::from(source.file_path.clone()),
            *platform,
            source.workspace_id.clone(),
            source.owner_user_id.clone(),
            operator,
            source.user_note.clone(),
            source.ai_note.clone(),
            source.ai_model.clone(),
            state.runtime_config.as_ref(),
        ) {
            Ok(snapshot) => {
                preflight_results.push(json!({
                    "platform": platform.as_str(),
                    "result": { "ok": true, "preflight": snapshot.details }
                }));
                preflight_ok.push((*platform, snapshot));
            }
            Err(err) => preflight_results.push(json!({
                "platform": platform.as_str(),
                "result": err.to_output()
            })),
        }
    }

    if preflight_ok.len() != platforms.len() {
        let message = build_multi_schedule_failure_message(preflight_results.as_slice());
        return Json(json!({
            "ok": false,
            "mode": "job_schedule_multi",
            "job_id": payload.id,
            "action_group_id": common_action_group_id,
            "content_group_id": common_content_group_id,
            "message": message,
            "results": preflight_results
        }));
    }

    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => return Json(json!({ "ok": false, "message": message })),
    };
    let now = Utc::now().to_rfc3339();
    let timezone = payload.timezone.clone();
    let mut scheduled_ids: Vec<(String, PlatformArg)> = Vec::new();
    let tx_result = conn.transaction::<(), diesel::result::Error, _>(|tx| {
        for (platform, snapshot) in &preflight_ok {
            let publish_mode = if matches!(platform, PlatformArg::X) {
                Some("single")
            } else {
                None
            };
            let selected_platforms_json = selected_platforms_json(&[*platform]);

            let changed = sql_query(
                "UPDATE jobs
                 SET action_group_id = ?,
                     content_group_id = ?,
                     status = 'scheduled',
                     platform = ?,
                     publish_mode = ?,
                     run_at_utc = ?,
                     timezone = ?,
                     status_reason = NULL,
                     operator = ?,
                     user_note = COALESCE(?, user_note),
                     ai_note = COALESCE(?, ai_note),
                     ai_model = COALESCE(?, ai_model),
                     selected_platforms = ?,
                     file_path = ?,
                     file_sha256 = ?,
                     text_sha256 = ?,
                     fingerprint = ?,
                     updated_at = ?,
                     version = version + 1,
                     synced_at = NULL,
                     modified_by = 'local'
                 WHERE asset_id = ? AND platform = ? AND deleted_at IS NULL",
            )
            .bind::<Text, _>(&common_action_group_id)
            .bind::<Text, _>(&common_content_group_id)
            .bind::<Text, _>(platform.as_str())
            .bind::<Nullable<Text>, _>(publish_mode)
            .bind::<Text, _>(&run_at_utc)
            .bind::<Nullable<Text>, _>(timezone.as_deref())
            .bind::<Text, _>(operator.as_str())
            .bind::<Nullable<Text>, _>(source.user_note.as_deref())
            .bind::<Nullable<Text>, _>(source.ai_note.as_deref())
            .bind::<Nullable<Text>, _>(source.ai_model.as_deref())
            .bind::<Text, _>(&selected_platforms_json)
            .bind::<Text, _>(&source.file_path)
            .bind::<Text, _>(&snapshot.file_sha256)
            .bind::<Text, _>(&snapshot.text_sha256)
            .bind::<Text, _>(&snapshot.fingerprint)
            .bind::<Text, _>(&now)
            .bind::<Text, _>(&source.asset_id)
            .bind::<Text, _>(platform.as_str())
            .execute(tx)?;

            if changed == 0 {
                let id = generate_id();
                sql_query(
                    "INSERT INTO jobs (
                        id, action_group_id, content_group_id, asset_id, kind, status, platform, publish_mode, workspace_id, owner_user_id, operator, user_note, ai_note, ai_model, tags, selected_platforms,
                        file_path, run_at_utc, timezone, status_reason, attempt_count, file_sha256, text_sha256, fingerprint,
                        created_at, updated_at, deleted_at, version, synced_at, modified_by
                     ) VALUES (
                        ?, ?, ?, ?, 'catalog', 'scheduled', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                        ?, ?, ?, NULL, 0, ?, ?, ?,
                        ?, ?, NULL, 1, NULL, 'local'
                     )",
                )
                .bind::<Text, _>(&id)
                .bind::<Text, _>(&common_action_group_id)
                .bind::<Text, _>(&common_content_group_id)
                .bind::<Text, _>(&source.asset_id)
                .bind::<Text, _>(platform.as_str())
                .bind::<Nullable<Text>, _>(publish_mode)
                .bind::<Text, _>(&source.workspace_id)
                .bind::<Nullable<Text>, _>(source.owner_user_id.as_deref())
                .bind::<Text, _>(operator.as_str())
                .bind::<Nullable<Text>, _>(source.user_note.as_deref())
                .bind::<Nullable<Text>, _>(source.ai_note.as_deref())
                .bind::<Nullable<Text>, _>(source.ai_model.as_deref())
                .bind::<Text, _>(&source.tags)
                .bind::<Text, _>(&selected_platforms_json)
                .bind::<Text, _>(&source.file_path)
                .bind::<Text, _>(&run_at_utc)
                .bind::<Nullable<Text>, _>(timezone.as_deref())
                .bind::<Text, _>(&snapshot.file_sha256)
                .bind::<Text, _>(&snapshot.text_sha256)
                .bind::<Text, _>(&snapshot.fingerprint)
                .bind::<Text, _>(&now)
                .bind::<Text, _>(&now)
                .execute(tx)?;
                scheduled_ids.push((id, *platform));
            } else {
                let mut rows: Vec<ReadyJobRow> = sql_query(
                    "SELECT id FROM jobs WHERE asset_id = ? AND platform = ? AND deleted_at IS NULL LIMIT 1",
                )
                .bind::<Text, _>(&source.asset_id)
                .bind::<Text, _>(platform.as_str())
                .load(tx)?;
                let Some(row) = rows.pop() else {
                    return Err(diesel::result::Error::NotFound);
                };
                scheduled_ids.push((row.id, *platform));
            }
        }
        let _ = sql_query("DELETE FROM jobs WHERE asset_id = ? AND status = 'ready'")
            .bind::<Text, _>(&source.asset_id)
            .execute(tx)?;
        Ok(())
    });

    if let Err(err) = tx_result {
        return Json(json!({
            "ok": false,
            "mode": "job_schedule_multi",
            "job_id": payload.id,
            "action_group_id": common_action_group_id,
            "content_group_id": common_content_group_id,
            "message": format!("Failed to schedule selected platforms. No changes were saved: {err}")
        }));
    }

    let mut result_items: Vec<Value> = Vec::new();
    for (id, platform) in &scheduled_ids {
        let job_value = list_jobs_by_id(state.db_path.as_path(), id)
            .ok()
            .flatten()
            .unwrap_or_else(|| json!({ "id": id }));
        result_items.push(json!({
            "platform": platform.as_str(),
            "result": {
                "ok": true,
                "job": job_value
            }
        }));
    }

    Json(json!({
        "ok": true,
        "mode": "job_schedule_multi",
        "job_id": payload.id,
        "action_group_id": common_action_group_id,
        "content_group_id": common_content_group_id,
        "results": result_items
    }))
}

async fn set_scheduled_time(
    State(state): State<ApiState>,
    Json(payload): Json<JobTimePayload>,
) -> Json<Value> {
    let run_at_utc = match parse_run_at_to_utc(payload.at.as_str()) {
        Ok(value) => value,
        Err(message) => {
            return Json(json!({
                "ok": false,
                "error_type": "validation_error",
                "message": message
            }))
        }
    };
    let mut conn = match open_db_by_path(state.db_path.as_path()) {
        Ok(conn) => conn,
        Err(message) => return Json(json!({ "ok": false, "message": message })),
    };
    let now = Utc::now().to_rfc3339();
    let changed = sql_query(
        "UPDATE jobs
         SET run_at_utc = ?,
             timezone = ?,
             updated_at = ?,
             version = version + 1,
             synced_at = NULL,
             modified_by = 'local'
         WHERE id = ? AND status = 'scheduled'",
    )
    .bind::<Text, _>(&run_at_utc)
    .bind::<Nullable<Text>, _>(payload.timezone.as_deref())
    .bind::<Text, _>(&now)
    .bind::<Text, _>(&payload.id)
    .execute(&mut conn);

    match changed {
        Ok(0) => Json(json!({
            "ok": false,
            "error_type": "validation_error",
            "message": "Only scheduled jobs can be rescheduled."
        })),
        Ok(_) => match list_jobs_by_id(state.db_path.as_path(), &payload.id) {
            Ok(Some(job)) => Json(json!({ "ok": true, "job": job })),
            Ok(None) => Json(json!({ "ok": true })),
            Err(message) => Json(json!({ "ok": false, "message": message })),
        },
        Err(err) => Json(json!({
            "ok": false,
            "error_type": "io_error",
            "message": format!("Failed to update scheduled time: {err}")
        })),
    }
}

async fn unschedule_job(
    State(state): State<ApiState>,
    Json(payload): Json<JobUnschedulePayload>,
) -> Json<Value> {
    let args = JobUnscheduleArgs {
        id: payload.id,
        reason: payload.reason,
    };
    match job_unschedule(args, state.runtime_config.as_ref()) {
        Ok(value) => Json(value),
        Err(err) => Json(json!(err.to_output())),
    }
}

async fn cancel_job(
    State(state): State<ApiState>,
    Json(payload): Json<JobCancelPayload>,
) -> Json<Value> {
    let args = JobCancelArgs {
        id: payload.id,
        reason: payload.reason,
    };
    match job_cancel(args, state.runtime_config.as_ref()) {
        Ok(value) => Json(value),
        Err(err) => Json(json!(err.to_output())),
    }
}

async fn spa_index(State(state): State<ApiState>) -> Response {
    let dist_path = match state.web_dist_path.as_ref() {
        Some(path) => path,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "web/dist not found. Run `pnpm --dir web build` first.",
            )
                .into_response()
        }
    };
    let index_path = dist_path.join("index.html");
    match fs::read(index_path) {
        Ok(bytes) => {
            let mut response = bytes.into_response();
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
            response
        }
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "web/dist/index.html not found. Run `pnpm --dir web build` first.",
        )
            .into_response(),
    }
}

async fn spa_asset(State(state): State<ApiState>, RoutePath(path): RoutePath<String>) -> Response {
    if path.starts_with("api/") || path == "health" {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }

    let dist_path = match state.web_dist_path.as_ref() {
        Some(path) => path,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "web/dist not found. Run `pnpm --dir web build` first.",
            )
                .into_response()
        }
    };
    let requested = sanitize_asset_path(&path).unwrap_or_else(|| PathBuf::from("index.html"));
    let file_path = dist_path.join(&requested);

    if file_path.is_file() {
        match fs::read(&file_path) {
            Ok(bytes) => {
                let content_type = static_content_type(file_path.as_path());
                let mut response = bytes.into_response();
                response
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
                return response;
            }
            Err(_) => return (StatusCode::NOT_FOUND, "Not found").into_response(),
        }
    }

    // SPA fallback for client routes.
    let index_path = dist_path.join("index.html");
    match fs::read(index_path) {
        Ok(bytes) => {
            let mut response = bytes.into_response();
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
            response
        }
        Err(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
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
    Uuid::new_v4().to_string()
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
            SELECT file_path, MAX(updated_at) AS max_updated_at
            FROM jobs
            WHERE status = 'ready'
            GROUP BY file_path
         ) latest
         ON latest.file_path = j.file_path
         AND latest.max_updated_at = j.updated_at
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

fn list_catalog_job_states(db_path: &Path) -> Result<Vec<CatalogJobState>, String> {
    let mut conn = open_db_by_path(db_path)?;
    let rows: Vec<CatalogJobStateRow> = sql_query(
        "SELECT file_path, status, platform
         FROM jobs
         WHERE deleted_at IS NULL
         ORDER BY updated_at DESC, created_at DESC",
    )
    .load(&mut conn)
    .map_err(|err| format!("Failed to load catalog job states: {err}"))?;

    let mut by_path: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut badges_by_path: BTreeMap<String, Vec<CatalogJobBadge>> = BTreeMap::new();

    for row in rows {
        let badge_key = format!(
            "{}|{}",
            row.status,
            row.platform.clone().unwrap_or_default().to_ascii_lowercase()
        );
        let key_set = by_path.entry(row.file_path.clone()).or_default();
        if key_set.insert(badge_key) {
            badges_by_path
                .entry(row.file_path)
                .or_default()
                .push(CatalogJobBadge {
                    status: row.status,
                    platform: row.platform,
                });
        }
    }

    let mut out: Vec<CatalogJobState> = badges_by_path
        .into_iter()
        .map(|(path, mut badges)| {
            badges.sort_by(|a, b| {
                let status_rank_cmp = catalog_status_rank(a.status.as_str())
                    .cmp(&catalog_status_rank(b.status.as_str()));
                if status_rank_cmp != std::cmp::Ordering::Equal {
                    return status_rank_cmp;
                }
                a.platform
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.platform.as_deref().unwrap_or(""))
            });
            CatalogJobState { path, badges }
        })
        .collect();

    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn catalog_status_rank(status: &str) -> usize {
    match status {
        "ready" => 0,
        "scheduled" => 1,
        "publishing" => 2,
        "published" => 3,
        "blocked" => 4,
        "failed" => 5,
        "canceled" => 6,
        "disabled" => 7,
        _ => 99,
    }
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

fn ready_job_id_for_asset(db_path: &Path, asset_id: &str) -> Option<String> {
    let mut conn = open_db_by_path(db_path).ok()?;
    let mut rows: Vec<ReadyJobRow> = sql_query(
        "SELECT id
         FROM jobs
         WHERE status = 'ready' AND platform IS NULL AND asset_id = ?
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind::<Text, _>(asset_id)
    .load(&mut conn)
    .ok()?;
    rows.pop().map(|row| row.id)
}

fn asset_id_for_paths(db_path: &Path, canonical_path: &str, requested_path: &str) -> Option<String> {
    let mut conn = open_db_by_path(db_path).ok()?;
    let mut rows: Vec<RevivableJobRow> = sql_query(
        "SELECT id, status, asset_id
         FROM jobs
         WHERE file_path = ? OR file_path = ?
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1",
    )
    .bind::<Text, _>(canonical_path)
    .bind::<Text, _>(requested_path)
    .load(&mut conn)
    .ok()?;
    rows.pop().map(|row| row.asset_id)
}

fn latest_revivable_job_for_path(
    db_path: &Path,
    canonical_path: &str,
    requested_path: &str,
) -> Option<RevivableJobRow> {
    let mut conn = open_db_by_path(db_path).ok()?;
    let mut rows: Vec<RevivableJobRow> = sql_query(
        "SELECT id, status, asset_id
         FROM jobs
         WHERE (file_path = ? OR file_path = ?)
           AND status IN ('canceled', 'blocked', 'disabled', 'failed')
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1",
    )
    .bind::<Text, _>(canonical_path)
    .bind::<Text, _>(requested_path)
    .load(&mut conn)
    .ok()?;
    rows.pop()
}

fn purge_terminal_jobs_for_path(
    conn: &mut SqliteConnection,
    canonical_path: &str,
    requested_path: &str,
    keep_id: Option<&str>,
) -> usize {
    let mut removed = 0usize;
    match keep_id {
        Some(id) => {
            removed += sql_query(
                "DELETE FROM jobs
                 WHERE file_path = ?
                   AND status IN ('canceled', 'blocked', 'disabled', 'failed')
                   AND id <> ?",
            )
            .bind::<Text, _>(canonical_path)
            .bind::<Text, _>(id)
            .execute(conn)
            .unwrap_or(0);
            if requested_path != canonical_path {
                removed += sql_query(
                    "DELETE FROM jobs
                     WHERE file_path = ?
                       AND status IN ('canceled', 'blocked', 'disabled', 'failed')
                       AND id <> ?",
                )
                .bind::<Text, _>(requested_path)
                .bind::<Text, _>(id)
                .execute(conn)
                .unwrap_or(0);
            }
        }
        None => {
            removed += sql_query(
                "DELETE FROM jobs
                 WHERE file_path = ?
                   AND status IN ('canceled', 'blocked', 'disabled', 'failed')",
            )
            .bind::<Text, _>(canonical_path)
            .execute(conn)
            .unwrap_or(0);
            if requested_path != canonical_path {
                removed += sql_query(
                    "DELETE FROM jobs
                     WHERE file_path = ?
                       AND status IN ('canceled', 'blocked', 'disabled', 'failed')",
                )
                .bind::<Text, _>(requested_path)
                .execute(conn)
                .unwrap_or(0);
            }
        }
    }

    removed
}

fn list_jobs_for_path(db_path: &Path, path: &Path) -> Result<Vec<Value>, String> {
    let mut conn = open_db_by_path(db_path)?;
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_str = canonical.to_string_lossy().to_string();
    let requested_str = path.to_string_lossy().to_string();

    let rows: Vec<FileJobRow> = sql_query(
        "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
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
            let selected_platforms = selected_platforms_from_json(&row.selected_platforms);
            json!({
                "id": row.id,
                "action_group_id": row.action_group_id,
                "content_group_id": row.content_group_id,
                "asset_id": row.asset_id,
                "kind": row.kind,
                "status": row.status,
                "platform": row.platform,
                "publish_mode": row.publish_mode,
                "workspace_id": row.workspace_id,
                "owner_user_id": row.owner_user_id,
                "operator": row.operator,
                "user_note": row.user_note,
                "ai_note": row.ai_note,
                "ai_model": row.ai_model,
                "tags": tags,
                "selected_platforms": selected_platforms,
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

fn list_jobs_by_status(db_path: &Path, status: &str) -> Result<Vec<Value>, String> {
    let mut conn = open_db_by_path(db_path)?;
    let rows: Vec<FileJobRow> = sql_query(
        "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
         FROM jobs
         WHERE status = ?
         ORDER BY COALESCE(run_at_utc, created_at) ASC, updated_at ASC",
    )
    .bind::<Text, _>(status)
    .load(&mut conn)
    .map_err(|err| format!("Failed to load jobs by status: {err}"))?;

    Ok(rows.into_iter().map(file_job_row_to_json).collect())
}

fn list_jobs_by_id(db_path: &Path, id: &str) -> Result<Option<Value>, String> {
    let mut conn = open_db_by_path(db_path)?;
    let mut rows: Vec<FileJobRow> = sql_query(
        "SELECT id,action_group_id,content_group_id,asset_id,kind,status,platform,publish_mode,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path,run_at_utc,timezone,status_reason,attempt_count,file_sha256,text_sha256,fingerprint,created_at,updated_at
         FROM jobs
         WHERE id = ?
         LIMIT 1",
    )
    .bind::<Text, _>(id)
    .load(&mut conn)
    .map_err(|err| format!("Failed to load job by id: {err}"))?;

    Ok(rows.pop().map(file_job_row_to_json))
}

fn file_job_row_to_json(row: FileJobRow) -> Value {
    let tags: Value = serde_json::from_str(&row.tags).unwrap_or_else(|_| json!([]));
    let selected_platforms = selected_platforms_from_json(&row.selected_platforms);
    json!({
        "id": row.id,
        "action_group_id": row.action_group_id,
        "content_group_id": row.content_group_id,
        "asset_id": row.asset_id,
        "kind": row.kind,
        "status": row.status,
        "platform": row.platform,
        "publish_mode": row.publish_mode,
        "workspace_id": row.workspace_id,
        "owner_user_id": row.owner_user_id,
        "operator": row.operator,
        "user_note": row.user_note,
        "ai_note": row.ai_note,
        "ai_model": row.ai_model,
        "tags": tags,
        "selected_platforms": selected_platforms,
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
}

fn parse_platform_arg(raw: &str) -> Option<PlatformArg> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "linkedin" => Some(PlatformArg::Linkedin),
        "x" => Some(PlatformArg::X),
        _ => None,
    }
}

fn parse_run_at_to_utc(raw: &str) -> Result<String, String> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
        .map_err(|err| format!("Invalid datetime format. Expected RFC3339: {err}"))
}

fn normalize_platform_list(raw_items: &[String]) -> Result<Vec<PlatformArg>, String> {
    let mut out: Vec<PlatformArg> = Vec::new();
    for item in raw_items {
        let Some(platform) = parse_platform_arg(item.as_str()) else {
            return Err(format!("Invalid platform: {item}. Use linkedin or x."));
        };
        if !out.contains(&platform) {
            out.push(platform);
        }
    }
    Ok(out)
}

fn build_multi_schedule_failure_message(results: &[Value]) -> String {
    let mut failed: Vec<String> = Vec::new();
    for item in results {
        let platform = item
            .get("platform")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let result_ok = item
            .get("result")
            .and_then(|result| result.get("ok"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if result_ok {
            continue;
        }
        let reason = item
            .get("result")
            .and_then(|result| result.get("message"))
            .and_then(|value| value.as_str())
            .or_else(|| {
                item.get("result")
                    .and_then(|result| result.get("reason"))
                    .and_then(|value| value.as_str())
            })
            .or_else(|| {
                item.get("result")
                    .and_then(|result| result.get("error"))
                    .and_then(|error| error.get("message"))
                    .and_then(|value| value.as_str())
            })
            .unwrap_or("unknown error");
        failed.push(format!("{platform}: {reason}"));
    }

    if failed.is_empty() {
        "One or more selected platforms failed to schedule.".to_string()
    } else {
        format!(
            "One or more selected platforms failed to schedule ({})",
            failed.join("; ")
        )
    }
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

fn load_ready_source_job(db_path: &Path, id: &str) -> Result<Option<ReadySourceJobRow>, String> {
    let mut conn = open_db_by_path(db_path)?;
    let mut rows: Vec<ReadySourceJobRow> = sql_query(
        "SELECT status,content_group_id,asset_id,workspace_id,owner_user_id,operator,user_note,ai_note,ai_model,tags,selected_platforms,file_path
         FROM jobs
         WHERE id = ?
         LIMIT 1",
    )
    .bind::<Text, _>(id)
    .load(&mut conn)
    .map_err(|err| format!("Failed to read ready source job: {err}"))?;

    Ok(rows.pop())
}

fn sanitize_asset_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(trimmed);
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return None;
    }
    Some(candidate)
}

fn static_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
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
