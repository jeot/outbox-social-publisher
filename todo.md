# Outbox Roadmap

## Phase 1 - Foundation (Local CLI + LinkedIn MVP)

- [x] Choose Rust workspace structure and CLI framework.
- [x] Define minimal content file format for `publish-now` (single text file).
- [x] Define CLI output contract as JSON (machine-readable by default), including:
- [x] Success fields (`ok`, `platform`, `post_id`, `post_url`, `request_id`, `published_at`).
- [x] Error fields (`ok`, `error_type`, `message`, `http_status`, `api_error`, `retryable`).
- [x] Define process exit code contract (success vs failure categories).
- [x] Define request timeout policy (connect timeout, request timeout, max retry timeout).
- [x] Add config loading strategy:
- [x] Secrets in `.env` and always keep `.env.example` in sync.
- [x] Non-secret defaults in `config.toml` (timeouts, log level, output mode, paths).
- [x] Implement `outbox publish linkedin --file <path>` command.
- [x] Implement LinkedIn OAuth setup flow for a single personal account.
- [x] Add non-interactive auth commands: `guide`, `login`, `exchange --code --state`, `whoami`.
- [x] Store OAuth tokens in `.env` (local, not committed).
- [x] Add automatic access-token refresh using refresh token.
- [x] Implement LinkedIn "publish text post as-is" adapter.
- [x] Add duplicate-publish guard for immediate retries.
- [x] Add deterministic idempotency key generation for publish requests.
- [x] Return API error details transparently in JSON output.
- [x] Write setup guide for another user on macOS, Linux, and Windows.
- [ ] Build and test release binaries for macOS, Linux, and Windows.

## Phase 2 - Scheduling and Background Worker (LinkedIn)

- [ ] Define local schedule/state file format and job states (`ready`, `scheduled`, `publishing`, `published`, `failed`).
- [ ] Add `outbox schedule add` and `outbox schedule list` commands.
- [ ] Add worker mode to process due jobs from local schedule storage.
- [ ] Add idempotency key strategy to prevent duplicate LinkedIn posts.
- [ ] Add retry policy with capped attempts and error logging.
- [ ] Add per-job audit log file for publish attempts (JSON lines).
- [ ] Add OS integration docs for running worker in background (`launchd`, `systemd`, Task Scheduler).
- [ ] Add dry-run mode for scheduler verification without publishing.
- [ ] Add volume checkpoint for persistence strategy:
- [ ] Stay file-first for early scale.
- [ ] Define threshold and migration trigger to SQLite (for example around thousands of published items and growing query latency).

## Phase 3 - X Platform Support

- [ ] Implement X platform adapter behind shared publisher interface.
- [ ] Add X authentication setup and token storage.
- [ ] Add `outbox publish x --file <path>` command path.
- [ ] Add X character-length validation and thread mode decision.
- [ ] Integrate X into scheduler flow and retry/idempotency behavior.

## Phase 4 - Substack Support

- [ ] Evaluate and choose Substack integration strategy (official path or maintained community integration).
- [ ] Implement Substack adapter behind shared publisher interface.
- [ ] Add `outbox publish substack --file <path>` command path.
- [ ] Add scheduler integration for Substack with clear failure handling.
- [ ] Isolate Substack-specific dependency and fallback path in docs.

## Phase 5 - Instagram Support

- [ ] Implement Instagram adapter behind shared publisher interface.
- [ ] Add media validation requirements (image/video format and size checks).
- [ ] Add `outbox publish instagram --file <path> --media <path>` command path.
- [ ] Add scheduler integration for Instagram and independent failure handling.
- [ ] Add platform-specific preflight checks before scheduling.

## Phase 6 - Remote Control (Optional, Local-First Preserved)

- [ ] Add command-ingest interface for remote triggers (Telegram bot or simple HTTP endpoint).
- [ ] Keep execution on local machine while online service only sends commands.
- [ ] Add authenticated command queue with replay protection.
- [ ] Add status callbacks so remote client can see publish result.
- [ ] Document behavior when local machine is offline.

## Phase 7 - Dashboard (Optional)

- [ ] Expose local API from Rust worker for dashboard consumption.
- [ ] Build TypeScript dashboard for content list, schedule view, and publish actions.
- [ ] Add sync view for local file status and job status.
- [ ] Keep dashboard as interface layer; keep publish logic in Rust core.
