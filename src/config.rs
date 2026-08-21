use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

const DEFAULT_DB_PATH: &str = ".publo/publo.db";

#[derive(Debug, Deserialize)]
struct FileConfig {
    output: Option<OutputConfig>,
    timeouts: Option<TimeoutConfig>,
    signature: Option<SignatureConfigFile>,
    platform: Option<PlatformConfig>,
    media: Option<MediaConfigFile>,
    db: Option<DbConfigFile>,
    api: Option<ApiConfigFile>,
    catalog: Option<CatalogConfigFile>,
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
}

pub(crate) fn load_config() -> RuntimeConfig {
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
        db_path: PathBuf::from(DEFAULT_DB_PATH),
        api_host: "127.0.0.1".to_string(),
        api_port: 8787,
        catalog_roots: Vec::new(),
    };

    let Some(config_path) = resolve_project_file("config.toml") else {
        return defaults;
    };

    let Ok(raw) = fs::read_to_string(config_path) else {
        return defaults;
    };

    let Ok(file_config) = toml::from_str::<FileConfig>(&raw) else {
        return defaults;
    };

    let pretty_json = file_config
        .output
        .and_then(|o| o.pretty_json)
        .unwrap_or(defaults.pretty_json);

    let connect_timeout = file_config
        .timeouts
        .as_ref()
        .and_then(|t| t.connect_seconds)
        .map(Duration::from_secs)
        .unwrap_or(defaults.connect_timeout);

    let request_timeout = file_config
        .timeouts
        .as_ref()
        .and_then(|t| t.request_seconds)
        .map(Duration::from_secs)
        .unwrap_or(defaults.request_timeout);

    let global_signature = to_signature_layer(file_config.signature.as_ref());
    let linkedin_signature = to_signature_layer(
        file_config
            .platform
            .as_ref()
            .and_then(|p| p.linkedin.as_ref())
            .and_then(|p| p.signature.as_ref()),
    );
    let x_signature = to_signature_layer(
        file_config
            .platform
            .as_ref()
            .and_then(|p| p.x.as_ref())
            .and_then(|p| p.signature.as_ref()),
    );
    let media_lookup_paths = file_config
        .media
        .as_ref()
        .and_then(|m| m.lookup_paths.as_ref())
        .map(|items| items.iter().map(PathBuf::from).collect())
        .unwrap_or_else(Vec::new);
    let db_path = file_config
        .db
        .as_ref()
        .and_then(|d| d.path.as_ref())
        .map(PathBuf::from)
        .unwrap_or_else(|| defaults.db_path.clone());
    let api_host = file_config
        .api
        .as_ref()
        .and_then(|a| a.host.as_ref())
        .cloned()
        .unwrap_or_else(|| defaults.api_host.clone());
    let api_port = file_config
        .api
        .as_ref()
        .and_then(|a| a.port)
        .unwrap_or(defaults.api_port);
    let catalog_roots = file_config
        .catalog
        .as_ref()
        .and_then(|c| c.roots.as_ref())
        .map(|items| items.iter().map(PathBuf::from).collect())
        .unwrap_or_else(Vec::new);

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
    }
}

pub(crate) fn resolve_project_file(relative: &str) -> Option<PathBuf> {
    let cwd_candidate = PathBuf::from(relative);
    if cwd_candidate.exists() {
        return Some(cwd_candidate);
    }
    let repo_candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    if repo_candidate.exists() {
        return Some(repo_candidate);
    }
    None
}

fn to_signature_layer(cfg: Option<&SignatureConfigFile>) -> SignatureLayer {
    SignatureLayer {
        enabled: cfg.and_then(|c| c.enabled),
        text: cfg.and_then(|c| c.text.clone()),
    }
}
