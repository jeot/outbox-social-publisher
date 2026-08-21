use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

const DEFAULT_HOME_DIR_NAME: &str = ".publo.so";
const DEFAULT_WORKSPACE_ID: &str = "default";
const GLOBAL_CONFIG_FILE_NAME: &str = "config.toml";
const WORKSPACE_CONFIG_FILE_NAME: &str = "config.toml";
const WORKSPACE_ENV_FILE_NAME: &str = ".env";

#[derive(Debug, Deserialize)]
struct GlobalConfigFile {
    output: Option<OutputConfig>,
    timeouts: Option<TimeoutConfig>,
    api: Option<ApiConfigFile>,
    workspace: Option<WorkspaceConfigSelector>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceFileConfig {
    workspace: Option<WorkspaceMetaConfig>,
    output: Option<OutputConfig>,
    timeouts: Option<TimeoutConfig>,
    signature: Option<SignatureConfigFile>,
    platform: Option<PlatformConfig>,
    media: Option<MediaConfigFile>,
    db: Option<DbConfigFile>,
    catalog: Option<CatalogConfigFile>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMetaConfig {
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceConfigSelector {
    default: Option<String>,
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

#[derive(Debug, Deserialize, Clone)]
struct SignatureConfigFile {
    enabled: Option<bool>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlatformConfig {
    linkedin: Option<PlatformEntryConfig>,
    x: Option<PlatformEntryConfig>,
}

#[derive(Debug, Deserialize)]
struct PlatformEntryConfig {
    signature: Option<SignatureConfigFile>,
}

#[derive(Debug, Deserialize)]
struct MediaConfigFile {
    lookup_paths: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct DbConfigFile {
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiConfigFile {
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct CatalogConfigFile {
    roots: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct SignatureLayer {
    pub(crate) enabled: Option<bool>,
    pub(crate) text: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePaths {
    pub(crate) publo_home: PathBuf,
    pub(crate) global_config_path: PathBuf,
    pub(crate) workspace_id: String,
    pub(crate) workspace_dir: PathBuf,
    pub(crate) workspace_config_path: PathBuf,
    pub(crate) env_path: PathBuf,
    pub(crate) runtime_dir: PathBuf,
    pub(crate) db_default_path: PathBuf,
    pub(crate) publish_log_path: PathBuf,
    pub(crate) linkedin_oauth_state_path: PathBuf,
    pub(crate) x_oauth_state_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct RuntimeConfig {
    pub(crate) pretty_json: bool,
    pub(crate) connect_timeout: Duration,
    pub(crate) request_timeout: Duration,
    pub(crate) global_signature: SignatureLayer,
    pub(crate) linkedin_signature: SignatureLayer,
    pub(crate) x_signature: SignatureLayer,
    pub(crate) media_lookup_paths: Vec<PathBuf>,
    pub(crate) db_path: PathBuf,
    pub(crate) api_host: String,
    pub(crate) api_port: u16,
    pub(crate) catalog_roots: Vec<PathBuf>,
    pub(crate) workspace_display_name: String,
    pub(crate) paths: RuntimePaths,
}

pub(crate) fn load_config() -> RuntimeConfig {
    let paths = resolve_runtime_paths();
    let defaults = RuntimeConfig {
        pretty_json: false,
        connect_timeout: Duration::from_secs(10),
        request_timeout: Duration::from_secs(30),
        global_signature: SignatureLayer {
            enabled: None,
            text: None,
        },
        linkedin_signature: SignatureLayer {
            enabled: None,
            text: None,
        },
        x_signature: SignatureLayer {
            enabled: None,
            text: None,
        },
        media_lookup_paths: Vec::new(),
        db_path: paths.db_default_path.clone(),
        api_host: "127.0.0.1".to_string(),
        api_port: 8787,
        catalog_roots: Vec::new(),
        workspace_display_name: default_workspace_display_name(&paths.workspace_id),
        paths: paths.clone(),
    };

    let global_config = load_global_config(&paths.global_config_path);
    let workspace_config = load_workspace_config(&paths.workspace_config_path);

    let pretty_json = workspace_config
        .as_ref()
        .and_then(|cfg| cfg.output.as_ref())
        .and_then(|o| o.pretty_json)
        .or_else(|| {
            global_config
                .as_ref()
                .and_then(|cfg| cfg.output.as_ref())
                .and_then(|o| o.pretty_json)
        })
        .unwrap_or(defaults.pretty_json);

    let connect_timeout = workspace_config
        .as_ref()
        .and_then(|cfg| cfg.timeouts.as_ref())
        .and_then(|t| t.connect_seconds)
        .or_else(|| {
            global_config
                .as_ref()
                .and_then(|cfg| cfg.timeouts.as_ref())
                .and_then(|t| t.connect_seconds)
        })
        .map(Duration::from_secs)
        .unwrap_or(defaults.connect_timeout);

    let request_timeout = workspace_config
        .as_ref()
        .and_then(|cfg| cfg.timeouts.as_ref())
        .and_then(|t| t.request_seconds)
        .or_else(|| {
            global_config
                .as_ref()
                .and_then(|cfg| cfg.timeouts.as_ref())
                .and_then(|t| t.request_seconds)
        })
        .map(Duration::from_secs)
        .unwrap_or(defaults.request_timeout);

    let api_host = global_config
        .as_ref()
        .and_then(|cfg| cfg.api.as_ref())
        .and_then(|a| a.host.as_ref())
        .cloned()
        .unwrap_or_else(|| defaults.api_host.clone());

    let api_port = global_config
        .as_ref()
        .and_then(|cfg| cfg.api.as_ref())
        .and_then(|a| a.port)
        .unwrap_or(defaults.api_port);

    let global_signature = to_signature_layer(
        workspace_config
            .as_ref()
            .and_then(|cfg| cfg.signature.as_ref()),
    );
    let linkedin_signature = to_signature_layer(
        workspace_config
            .as_ref()
            .and_then(|cfg| cfg.platform.as_ref())
            .and_then(|p| p.linkedin.as_ref())
            .and_then(|p| p.signature.as_ref()),
    );
    let x_signature = to_signature_layer(
        workspace_config
            .as_ref()
            .and_then(|cfg| cfg.platform.as_ref())
            .and_then(|p| p.x.as_ref())
            .and_then(|p| p.signature.as_ref()),
    );

    let workspace_config_dir = paths.workspace_config_path.parent().unwrap_or(&paths.workspace_dir);

    let media_lookup_paths = workspace_config
        .as_ref()
        .and_then(|cfg| cfg.media.as_ref())
        .and_then(|m| m.lookup_paths.as_ref())
        .map(|items| {
            items
                .iter()
                .map(|item| resolve_path_from_config(workspace_config_dir, item))
                .collect()
        })
        .unwrap_or_else(Vec::new);

    let db_path = workspace_config
        .as_ref()
        .and_then(|cfg| cfg.db.as_ref())
        .and_then(|d| d.path.as_ref())
        .map(|raw| resolve_path_from_config(workspace_config_dir, raw))
        .unwrap_or_else(|| defaults.db_path.clone());

    let catalog_roots = workspace_config
        .as_ref()
        .and_then(|cfg| cfg.catalog.as_ref())
        .and_then(|c| c.roots.as_ref())
        .map(|items| {
            items
                .iter()
                .map(|item| resolve_path_from_config(workspace_config_dir, item))
                .collect()
        })
        .unwrap_or_else(Vec::new);

    let workspace_display_name = workspace_config
        .as_ref()
        .and_then(|cfg| cfg.workspace.as_ref())
        .and_then(|w| w.display_name.as_ref())
        .cloned()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default_workspace_display_name(&paths.workspace_id));

    RuntimeConfig {
        pretty_json,
        connect_timeout,
        request_timeout,
        global_signature,
        linkedin_signature,
        x_signature,
        media_lookup_paths,
        db_path,
        api_host,
        api_port,
        catalog_roots,
        workspace_display_name,
        paths,
    }
}

pub(crate) fn resolve_runtime_paths() -> RuntimePaths {
    let publo_home = resolve_publo_home();
    let global_config_path = resolve_global_config_path(&publo_home);
    let workspace_id = resolve_workspace_id(&global_config_path);
    resolve_runtime_paths_for_workspace_id(publo_home, global_config_path, workspace_id)
}

pub(crate) fn resolve_runtime_paths_for_workspace(workspace_id: &str) -> RuntimePaths {
    let publo_home = resolve_publo_home();
    let global_config_path = resolve_global_config_path(&publo_home);
    resolve_runtime_paths_for_workspace_id(
        publo_home,
        global_config_path,
        normalize_workspace_id(workspace_id),
    )
}

pub(crate) fn resolve_publo_home() -> PathBuf {
    if let Ok(raw) = env::var("PUBLO_HOME") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return absolutize_path(expand_home_alias(trimmed));
        }
    }
    home_dir().join(DEFAULT_HOME_DIR_NAME)
}

fn resolve_runtime_paths_for_workspace_id(
    publo_home: PathBuf,
    global_config_path: PathBuf,
    workspace_id: String,
) -> RuntimePaths {
    let workspace_dir = publo_home.join("workspaces").join(&workspace_id);
    let workspace_config_path = workspace_dir.join(WORKSPACE_CONFIG_FILE_NAME);
    let env_path = env::var("PUBLO_ENV")
        .ok()
        .map(|raw| absolutize_path(expand_home_alias(raw.trim())))
        .unwrap_or_else(|| workspace_dir.join(WORKSPACE_ENV_FILE_NAME));
    let runtime_dir = workspace_dir.join("runtime");
    let db_default_path = workspace_dir.join("publo.db");
    let publish_log_path = workspace_dir.join("publish-log.jsonl");
    let linkedin_oauth_state_path = runtime_dir.join("linkedin_oauth_state.json");
    let x_oauth_state_path = runtime_dir.join("x_oauth_state.json");

    RuntimePaths {
        publo_home,
        global_config_path,
        workspace_id,
        workspace_dir,
        workspace_config_path,
        env_path,
        runtime_dir,
        db_default_path,
        publish_log_path,
        linkedin_oauth_state_path,
        x_oauth_state_path,
    }
}

fn resolve_global_config_path(publo_home: &Path) -> PathBuf {
    if let Ok(raw) = env::var("PUBLO_CONFIG") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return absolutize_path(expand_home_alias(trimmed));
        }
    }
    publo_home.join(GLOBAL_CONFIG_FILE_NAME)
}

fn resolve_workspace_id(global_config_path: &Path) -> String {
    if let Ok(raw) = env::var("PUBLO_WORKSPACE") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return normalize_workspace_id(trimmed);
        }
    }

    load_global_config(global_config_path)
        .and_then(|cfg| cfg.workspace.and_then(|w| w.default))
        .map(|w| normalize_workspace_id(&w))
        .unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string())
}

pub(crate) fn normalize_workspace_id(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return DEFAULT_WORKSPACE_ID.to_string();
    }

    let mut out = String::new();
    let mut prev_dash = false;
    for ch in trimmed.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }

    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        DEFAULT_WORKSPACE_ID.to_string()
    } else {
        out
    }
}

pub(crate) fn default_workspace_display_name(workspace_id: &str) -> String {
    let cleaned = workspace_id.trim();
    if cleaned.is_empty() {
        return "Default".to_string();
    }
    cleaned
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    for ch in first.to_uppercase() {
        out.push(ch);
    }
    out.push_str(chars.as_str());
    out
}

fn load_global_config(path: &Path) -> Option<GlobalConfigFile> {
    let raw = fs::read_to_string(path).ok()?;
    toml::from_str::<GlobalConfigFile>(&raw).ok()
}

fn load_workspace_config(path: &Path) -> Option<WorkspaceFileConfig> {
    let raw = fs::read_to_string(path).ok()?;
    toml::from_str::<WorkspaceFileConfig>(&raw).ok()
}

fn resolve_path_from_config(config_dir: &Path, raw: &str) -> PathBuf {
    let expanded = expand_home_alias(raw.trim());
    if expanded.is_absolute() {
        return expanded;
    }
    config_dir.join(expanded)
}

fn expand_home_alias(raw: &str) -> PathBuf {
    if raw == "~" {
        return home_dir();
    }
    if let Some(stripped) = raw.strip_prefix("~/") {
        return home_dir().join(stripped);
    }
    PathBuf::from(raw)
}

fn absolutize_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    env::current_dir()
        .map(|cwd| cwd.join(path.clone()))
        .unwrap_or(path)
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn to_signature_layer(cfg: Option<&SignatureConfigFile>) -> SignatureLayer {
    SignatureLayer {
        enabled: cfg.and_then(|c| c.enabled),
        text: cfg.and_then(|c| c.text.clone()),
    }
}
