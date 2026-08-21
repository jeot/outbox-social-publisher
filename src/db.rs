use std::fs;

use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::config::RuntimeConfig;
use crate::errors::AppError;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

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
