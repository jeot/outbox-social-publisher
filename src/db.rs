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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(QueryableByName)]
    struct PublishModeRow {
        #[diesel(sql_type = Text)]
        publish_mode: String,
    }

    #[test]
    fn publish_mode_migration_removes_allowlist_and_preserves_existing_data() {
        let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory database");
        conn.run_next_migration(MIGRATIONS)
            .expect("run initial migration");
        sql_query(
            "INSERT INTO workspace_meta (singleton, workspace_id) VALUES (1, 'workspace-test')",
        )
        .execute(&mut conn)
        .expect("insert workspace identity");
        sql_query(
            "INSERT INTO jobs (
                id, action_group_id, content_group_id, asset_id, kind, status,
                platform, publish_mode, workspace_id, selected_platforms,
                file_path, run_at_utc
             ) VALUES (
                'job-x', 'action-x', 'content-x', 'asset-x', 'catalog',
                'published', 'x', 'single', 'workspace-test', '[\"x\"]',
                '/tmp/x.md', '2026-08-31T00:00:00Z'
             )",
        )
        .execute(&mut conn)
        .expect("insert existing job");
        sql_query(
            "INSERT INTO publish_attempts (
                id, job_id, attempt_no, platform, workspace_id, trigger_mode,
                started_at, success
             ) VALUES (
                'attempt-x', 'job-x', 1, 'x', 'workspace-test', 'worker',
                '2026-08-31T00:00:00Z', 1
             )",
        )
        .execute(&mut conn)
        .expect("insert existing attempt");

        conn.run_next_migration(MIGRATIONS)
            .expect("run open publish-mode migration");

        let jobs: CountRow = sql_query("SELECT COUNT(*) AS count FROM jobs")
            .get_result(&mut conn)
            .expect("count preserved jobs");
        let attempts: CountRow = sql_query("SELECT COUNT(*) AS count FROM publish_attempts")
            .get_result(&mut conn)
            .expect("count preserved attempts");
        let existing_mode: PublishModeRow =
            sql_query("SELECT publish_mode FROM jobs WHERE id = 'job-x'")
                .get_result(&mut conn)
                .expect("read preserved publish mode");
        assert_eq!(jobs.count, 1);
        assert_eq!(attempts.count, 1);
        assert_eq!(existing_mode.publish_mode, "single");

        sql_query(
            "INSERT INTO jobs (
                id, action_group_id, content_group_id, asset_id, kind, status,
                platform, publish_mode, workspace_id, selected_platforms,
                file_path, run_at_utc
             ) VALUES (
                'job-substack', 'action-substack', 'content-substack',
                'asset-substack', 'catalog', 'scheduled', 'substack', 'note',
                'workspace-test', '[\"substack\"]', '/tmp/substack.md',
                '2026-09-01T00:00:00Z'
             )",
        )
        .execute(&mut conn)
        .expect("insert Substack Note job after migration");

        sql_query(
            "INSERT INTO jobs (
                id, action_group_id, content_group_id, asset_id, kind, status,
                platform, publish_mode, workspace_id, selected_platforms,
                file_path, run_at_utc
             ) VALUES (
                'job-future', 'action-future', 'content-future', 'asset-future',
                'catalog', 'scheduled', 'substack', 'future-format',
                'workspace-test', '[\"substack\"]', '/tmp/future.md',
                '2026-09-02T00:00:00Z'
             )",
        )
        .execute(&mut conn)
        .expect("insert arbitrary future publish mode after migration");
    }
}
