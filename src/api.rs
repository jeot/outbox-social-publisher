use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::publish;

#[derive(Clone)]
struct ApiState {
    catalog_roots: Arc<Vec<PathBuf>>,
    media_lookup_paths: Arc<Vec<PathBuf>>,
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

pub async fn run_server(
    host: String,
    port: u16,
    catalog_roots: Vec<PathBuf>,
    media_lookup_paths: Vec<PathBuf>,
    pretty_json: bool,
) -> ExitCode {
    let state = ApiState {
        catalog_roots: Arc::new(catalog_roots),
        media_lookup_paths: Arc::new(media_lookup_paths),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/catalog/tree", get(catalog_tree))
        .route("/api/catalog/file", get(catalog_file))
        .route("/api/catalog/preview", get(catalog_preview))
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
    Json(json!({ "ok": true, "roots": roots }))
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
        Ok(content) => Json(json!({
            "ok": true,
            "path": requested.to_string_lossy(),
            "content": content
        })),
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
                path: path.to_string_lossy().to_string(),
                kind: "dir",
                children: Some(children),
            });
            continue;
        }
        if file_type.is_file() && name.to_ascii_lowercase().ends_with(".md") {
            nodes.push(CatalogNode {
                name,
                path: path.to_string_lossy().to_string(),
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

fn print_json(value: &Value, pretty: bool) {
    if pretty {
        if let Ok(serialized) = serde_json::to_string_pretty(value) {
            println!("{serialized}");
            return;
        }
    }
    println!("{}", value);
}
