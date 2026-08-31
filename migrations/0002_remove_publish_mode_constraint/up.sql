-- Remove the closed publish-mode allowlist so new content formats do not require
-- schema migrations. SQLite cannot drop a CHECK constraint in place, so rebuild
-- the two related tables inside the migration transaction while preserving rows.

CREATE TABLE jobs_v2 (
  id TEXT PRIMARY KEY,
  action_group_id TEXT NOT NULL,
  content_group_id TEXT NOT NULL,
  asset_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('catalog', 'quick')),
  status TEXT NOT NULL CHECK (
    status IN ('ready', 'scheduled', 'publishing', 'published', 'failed', 'blocked', 'canceled', 'disabled')
  ),
  platform TEXT,
  publish_mode TEXT,
  workspace_id TEXT NOT NULL DEFAULT 'default',
  owner_user_id TEXT,
  executor_hint TEXT CHECK (executor_hint IN ('local', 'remote')),
  operator TEXT NOT NULL DEFAULT 'user',
  user_note TEXT,
  ai_note TEXT,
  ai_model TEXT,
  tags TEXT NOT NULL DEFAULT '[]',
  selected_platforms TEXT NOT NULL DEFAULT '[]',
  file_path TEXT NOT NULL,
  run_at_utc TEXT,
  timezone TEXT,
  status_reason TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 0,
  publish_claim_token TEXT,
  publishing_started_at TEXT,
  published_at TEXT,
  last_error_type TEXT,
  last_error_message TEXT,
  last_http_status INTEGER,
  file_sha256 TEXT,
  text_sha256 TEXT,
  fingerprint TEXT,
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

INSERT INTO jobs_v2 (
  id, action_group_id, content_group_id, asset_id, kind, status, platform,
  publish_mode, workspace_id, owner_user_id, executor_hint, operator,
  user_note, ai_note, ai_model, tags, selected_platforms, file_path,
  run_at_utc, timezone, status_reason, attempt_count, publish_claim_token,
  publishing_started_at, published_at, last_error_type, last_error_message,
  last_http_status, file_sha256, text_sha256, fingerprint, created_at,
  updated_at, deleted_at, version, synced_at, modified_by
)
SELECT
  id, action_group_id, content_group_id, asset_id, kind, status, platform,
  publish_mode, workspace_id, owner_user_id, executor_hint, operator,
  user_note, ai_note, ai_model, tags, selected_platforms, file_path,
  run_at_utc, timezone, status_reason, attempt_count, publish_claim_token,
  publishing_started_at, published_at, last_error_type, last_error_message,
  last_http_status, file_sha256, text_sha256, fingerprint, created_at,
  updated_at, deleted_at, version, synced_at, modified_by
FROM jobs;

CREATE TABLE publish_attempts_v2 (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL,
  attempt_no INTEGER NOT NULL,
  platform TEXT NOT NULL,
  workspace_id TEXT NOT NULL DEFAULT 'default',
  owner_user_id TEXT,
  trigger_mode TEXT NOT NULL CHECK (trigger_mode IN ('worker', 'manual')),
  claim_token TEXT,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  success INTEGER NOT NULL CHECK (success IN (0, 1)),
  error_type TEXT,
  error_message TEXT,
  http_status INTEGER,
  response_json TEXT,
  post_id TEXT,
  post_url TEXT,
  request_id TEXT,
  file_sha256 TEXT,
  text_sha256 TEXT,
  fingerprint TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
  synced_at TEXT,
  modified_by TEXT NOT NULL DEFAULT 'local',
  FOREIGN KEY (job_id) REFERENCES jobs_v2(id) ON DELETE CASCADE
);

INSERT INTO publish_attempts_v2 (
  id, job_id, attempt_no, platform, workspace_id, owner_user_id,
  trigger_mode, claim_token, started_at, finished_at, success, error_type,
  error_message, http_status, response_json, post_id, post_url, request_id,
  file_sha256, text_sha256, fingerprint, created_at, updated_at, deleted_at,
  version, synced_at, modified_by
)
SELECT
  id, job_id, attempt_no, platform, workspace_id, owner_user_id,
  trigger_mode, claim_token, started_at, finished_at, success, error_type,
  error_message, http_status, response_json, post_id, post_url, request_id,
  file_sha256, text_sha256, fingerprint, created_at, updated_at, deleted_at,
  version, synced_at, modified_by
FROM publish_attempts;

DROP TABLE publish_attempts;
DROP TABLE jobs;

ALTER TABLE jobs_v2 RENAME TO jobs;
ALTER TABLE publish_attempts_v2 RENAME TO publish_attempts;

CREATE INDEX idx_jobs_status_run_at ON jobs(status, run_at_utc);
CREATE INDEX idx_jobs_platform_status ON jobs(platform, status);
CREATE INDEX idx_jobs_workspace_status_run_at ON jobs(workspace_id, status, run_at_utc);
CREATE INDEX idx_jobs_action_group_id ON jobs(action_group_id);
CREATE INDEX idx_jobs_content_group_id ON jobs(content_group_id);
CREATE INDEX idx_jobs_asset_id ON jobs(asset_id);
CREATE INDEX idx_jobs_file_path ON jobs(file_path);
CREATE INDEX idx_jobs_file_sha256 ON jobs(file_sha256);
CREATE INDEX idx_jobs_fingerprint ON jobs(fingerprint);
CREATE INDEX idx_jobs_operator ON jobs(operator);
CREATE INDEX idx_jobs_sync_scan ON jobs(updated_at, deleted_at);
CREATE UNIQUE INDEX uq_jobs_asset_platform
  ON jobs(asset_id, platform)
  WHERE deleted_at IS NULL AND platform IS NOT NULL;
CREATE UNIQUE INDEX uq_jobs_asset_ready
  ON jobs(asset_id)
  WHERE deleted_at IS NULL AND status = 'ready' AND platform IS NULL;

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

CREATE INDEX idx_attempts_job_attempt ON publish_attempts(job_id, attempt_no);
CREATE INDEX idx_attempts_workspace_started ON publish_attempts(workspace_id, started_at);
CREATE INDEX idx_attempts_job_started ON publish_attempts(job_id, started_at);
CREATE INDEX idx_attempts_success_started ON publish_attempts(success, started_at);
CREATE INDEX idx_attempts_sync_scan ON publish_attempts(updated_at, deleted_at);
