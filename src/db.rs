use std::fs;

use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use diesel::{QueryableByName, RunQueryDsl, sql_query};
use diesel::sql_types::{Integer, Text};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::config::RuntimeConfig;
use crate::errors::AppError;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(QueryableByName)]
struct WorkspaceIdRow {
    #[diesel(sql_type = Text)]
    workspace_id: String,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = Integer)]
    count: i32,
}

pub(crate) fn ensure_db_ready(config: &RuntimeConfig) -> Result<(), AppError> {
    if let Some(parent) = config.db_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| AppError::Io {
            message: format!(
                "Failed to create DB directory {}: {err}",
                parent.display()
            ),
        })?;
    }

    let db_url = config.db_path.to_string_lossy().into_owned();
    let mut conn = SqliteConnection::establish(&db_url).map_err(|err| AppError::Io {
        message: format!(
            "Failed to open SQLite DB at {}: {err}",
            config.db_path.display()
        ),
    })?;

    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|err| AppError::Io {
            message: format!(
                "Failed to run SQLite migrations for {}: {err}",
                config.db_path.display()
            ),
        })?;

    let identities: Vec<WorkspaceIdRow> = sql_query(
        "SELECT workspace_id FROM workspace_meta WHERE singleton = 1",
    )
    .load(&mut conn)
    .map_err(|err| AppError::Io {
        message: format!("Failed to read database workspace identity: {err}"),
    })?;

    match identities.as_slice() {
        [] => {
            sql_query("INSERT INTO workspace_meta (singleton, workspace_id) VALUES (1, ?)")
                .bind::<Text, _>(&config.workspace_id)
                .execute(&mut conn)
                .map_err(|err| AppError::Io {
                    message: format!("Failed to initialize database workspace identity: {err}"),
                })?;
        }
        [identity] if identity.workspace_id == config.workspace_id => {}
        [identity] => return Err(AppError::Validation {
            message: format!(
                "Database belongs to workspace {} but config declares {}.",
                identity.workspace_id, config.workspace_id
            ),
            suggestion: Some("Select the matching workspace or use its own database.".to_string()),
            command: Some("publo paths".to_string()),
        }),
        _ => return Err(AppError::Validation {
            message: "Database has more than one workspace identity row.".to_string(),
            suggestion: Some("Use a clean workspace database.".to_string()),
            command: None,
        }),
    }

    for table in ["jobs", "publish_attempts"] {
        let count: CountRow = sql_query(format!(
            "SELECT COUNT(*) AS count FROM {table} WHERE workspace_id <> ?"
        ))
        .bind::<Text, _>(&config.workspace_id)
        .get_result(&mut conn)
        .map_err(|err| AppError::Io {
            message: format!("Failed to validate {table} workspace identity: {err}"),
        })?;
        if count.count > 0 {
            return Err(AppError::Validation {
                message: format!(
                    "Database contains {} {table} record(s) for a different workspace.",
                    count.count
                ),
                suggestion: Some("Use a clean database for this workspace.".to_string()),
                command: None,
            });
        }
    }

    Ok(())
}

pub(crate) fn open_db(config: &RuntimeConfig) -> Result<SqliteConnection, AppError> {
    let db_url = config.db_path.to_string_lossy().into_owned();
    SqliteConnection::establish(&db_url).map_err(|err| AppError::Io {
        message: format!(
            "Failed to open SQLite DB at {}: {err}",
            config.db_path.display()
        ),
    })
}
