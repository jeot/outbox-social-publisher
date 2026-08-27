-- Publo SQLite schema v1
-- Purpose: persist user decisions and execution history.
-- Note: "discovered" catalog scan results are intentionally NOT persisted.

CREATE TABLE IF NOT EXISTS jobs (
  id TEXT PRIMARY KEY,                         -- UUID
  action_group_id TEXT NOT NULL,                      -- UUID grouping jobs created in one user action
  content_group_id TEXT NOT NULL,              -- UUID grouping jobs from the same content lineage
  asset_id TEXT NOT NULL,                      -- Stable identity for a file asset (path can change)
  kind TEXT NOT NULL CHECK (kind IN ('catalog', 'quick')),
  status TEXT NOT NULL CHECK (
    status IN ('ready', 'scheduled', 'publishing', 'published', 'failed', 'blocked', 'canceled', 'disabled')
  ),

  -- Target info
  platform TEXT,
  publish_mode TEXT CHECK (publish_mode IN ('single', 'thread')),
  workspace_id TEXT NOT NULL DEFAULT 'default',
  owner_user_id TEXT,
  executor_hint TEXT CHECK (executor_hint IN ('local', 'remote')),
  operator TEXT NOT NULL DEFAULT 'user',
  user_note TEXT,
  ai_note TEXT,
  ai_model TEXT,
  tags TEXT NOT NULL DEFAULT '[]',
  selected_platforms TEXT NOT NULL DEFAULT '[]', -- Decision Queue selection before per-platform jobs exist

  -- Source content (always file-based; ad_hoc creation writes a file first)
  file_path TEXT NOT NULL,

  -- Scheduling
  run_at_utc TEXT,                             -- RFC3339 UTC timestamp
  timezone TEXT,                               -- Original user timezone label (e.g. Asia/Tehran)

  -- Lifecycle metadata
  status_reason TEXT,                          -- human-readable reason for blocked/canceled/failed transitions
  attempt_count INTEGER NOT NULL DEFAULT 0,
  last_error_type TEXT,
  last_error_message TEXT,
  last_http_status INTEGER,

  -- Content integrity / traceability
  file_sha256 TEXT,
  text_sha256 TEXT,
  fingerprint TEXT,

  -- Sync metadata
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
  synced_at TEXT,
  modified_by TEXT NOT NULL DEFAULT 'local',
  CHECK (
    ((status = 'ready')
    OR
    (status IN ('scheduled', 'publishing', 'published', 'failed', 'blocked', 'canceled', 'disabled') AND platform IS NOT NULL))
    AND
    (status <> 'scheduled' OR run_at_utc IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS idx_jobs_status_run_at
  ON jobs(status, run_at_utc);

CREATE INDEX IF NOT EXISTS idx_jobs_platform_status
  ON jobs(platform, status);

CREATE INDEX IF NOT EXISTS idx_jobs_workspace_status_run_at
  ON jobs(workspace_id, status, run_at_utc);

CREATE INDEX IF NOT EXISTS idx_jobs_action_group_id
  ON jobs(action_group_id);

CREATE INDEX IF NOT EXISTS idx_jobs_content_group_id
  ON jobs(content_group_id);

CREATE INDEX IF NOT EXISTS idx_jobs_asset_id
  ON jobs(asset_id);

CREATE INDEX IF NOT EXISTS idx_jobs_file_path
  ON jobs(file_path);

CREATE INDEX IF NOT EXISTS idx_jobs_file_sha256
  ON jobs(file_sha256);

CREATE INDEX IF NOT EXISTS idx_jobs_fingerprint
  ON jobs(fingerprint);

CREATE INDEX IF NOT EXISTS idx_jobs_operator
  ON jobs(operator);

CREATE INDEX IF NOT EXISTS idx_jobs_sync_scan
  ON jobs(updated_at, deleted_at);

CREATE UNIQUE INDEX IF NOT EXISTS uq_jobs_asset_platform
  ON jobs(asset_id, platform)
  WHERE deleted_at IS NULL AND platform IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_jobs_asset_ready
  ON jobs(asset_id)
  WHERE deleted_at IS NULL AND status = 'ready' AND platform IS NULL;

CREATE TABLE IF NOT EXISTS workspace_meta (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  workspace_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER jobs_workspace_identity_insert
BEFORE INSERT ON jobs
WHEN NOT EXISTS (SELECT 1 FROM workspace_meta WHERE singleton = 1)
  OR NEW.workspace_id <> (SELECT workspace_id FROM workspace_meta WHERE singleton = 1)
BEGIN
  SELECT RAISE(ABORT, 'job workspace_id does not match database workspace identity');
END;

CREATE TRIGGER jobs_workspace_identity_update
BEFORE UPDATE OF workspace_id ON jobs
WHEN NEW.workspace_id <> (SELECT workspace_id FROM workspace_meta WHERE singleton = 1)
BEGIN
  SELECT RAISE(ABORT, 'job workspace_id does not match database workspace identity');
END;

CREATE TABLE IF NOT EXISTS publish_attempts (
  id TEXT PRIMARY KEY,                         -- UUID
  job_id TEXT NOT NULL,
  attempt_no INTEGER NOT NULL,
  platform TEXT NOT NULL,
  workspace_id TEXT NOT NULL DEFAULT 'default',
  owner_user_id TEXT,
  trigger_mode TEXT NOT NULL CHECK (trigger_mode IN ('worker', 'manual')),

  started_at TEXT NOT NULL,
  finished_at TEXT,
  success INTEGER NOT NULL CHECK (success IN (0, 1)),

  error_type TEXT,
  error_message TEXT,
  http_status INTEGER,
  response_json TEXT,                          -- raw provider/output json snapshot

  post_id TEXT,
  post_url TEXT,
  request_id TEXT,

  file_sha256 TEXT,
  text_sha256 TEXT,
  fingerprint TEXT,

  -- Sync metadata
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
  synced_at TEXT,
  modified_by TEXT NOT NULL DEFAULT 'local',

  FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_attempts_job_attempt
  ON publish_attempts(job_id, attempt_no);

CREATE INDEX IF NOT EXISTS idx_attempts_workspace_started
  ON publish_attempts(workspace_id, started_at);

CREATE INDEX IF NOT EXISTS idx_attempts_job_started
  ON publish_attempts(job_id, started_at);

CREATE INDEX IF NOT EXISTS idx_attempts_success_started
  ON publish_attempts(success, started_at);

CREATE INDEX IF NOT EXISTS idx_attempts_sync_scan
  ON publish_attempts(updated_at, deleted_at);

CREATE TRIGGER attempts_workspace_identity_insert
BEFORE INSERT ON publish_attempts
WHEN NOT EXISTS (SELECT 1 FROM workspace_meta WHERE singleton = 1)
  OR NEW.workspace_id <> (SELECT workspace_id FROM workspace_meta WHERE singleton = 1)
BEGIN
  SELECT RAISE(ABORT, 'attempt workspace_id does not match database workspace identity');
END;

CREATE TRIGGER attempts_workspace_identity_update
BEFORE UPDATE OF workspace_id ON publish_attempts
WHEN NEW.workspace_id <> (SELECT workspace_id FROM workspace_meta WHERE singleton = 1)
BEGIN
  SELECT RAISE(ABORT, 'attempt workspace_id does not match database workspace identity');
END;
