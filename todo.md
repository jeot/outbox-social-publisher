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
- [x] Add token tooling commands: `token-status`, `token-refresh`.
- [x] Store OAuth tokens in `.env` (local, not committed).
- [x] Add automatic access-token refresh using refresh token.
- [x] Implement LinkedIn "publish text post as-is" adapter.
- [x] Escape LinkedIn little-text reserved characters before publish.
- [x] Add `--debug` mode to validate/preview publish payload without posting.
- [x] Add duplicate-publish guard for immediate retries.
- [x] Add deterministic idempotency key generation for publish requests.
- [x] Return API error details transparently in JSON output.
- [x] Write setup guide for another user on macOS, Linux, and Windows.

## Phase 2 - X Platform Support

- [x] Implement X platform adapter behind shared publisher interface.
- [x] Add guided X OAuth login (local callback server + browser flow) and token storage.
- [x] Add `outbox publish x --file <path>` command path.
- [x] Add shared duplicate guard + JSONL publish logging parity for X.
- [x] Add X preflight validation for weighted-length and self-serve cashtag rule.
- [x] Add X bypass flags: `--allow-cashtag`, `--allow-length`, `--allow-duplicate`, `--force`.
- [x] Improve X API error suggestions (credits depleted, cashtag-limit, generic 403 with local hints).
- [x] Add signature support (global + per-platform config, plus CLI overrides) for LinkedIn and X.
- [x] Add `file_sha256` + `text_sha256` tracking in output and publish logs.
- [ ] Add X token tooling parity commands (`auth x token-status`, `auth x token-refresh`).
- [ ] Integrate X into scheduler flow and retry/idempotency behavior.

## Phase 3 - Media (Images)

- [x] Define Obsidian-note parsing contract:
- [x] Publish text from section after last `---`.
- [x] Discover `![[...]]` media embeds from whole file (ordered).
- [x] Strip embed placeholders from publish text before send.
- [x] Resolve media from note folder first, then `config.toml` media lookup paths.
- [x] Validate media extension (`.png`, `.jpg`, `.jpeg`) and block on invalid/missing files.
- [x] Add LinkedIn single-image upload flow (`rest/images initializeUpload` + upload + `rest/posts` media content).
- [x] Add LinkedIn multi-image publish support (MultiImage API, 2-20 images).
- [ ] Add X image upload/publish support.

## Phase 4 - Scheduling and Background Worker (LinkedIn)

- [ ] Define local schedule/state file format and job states (`ready`, `scheduled`, `publishing`, `published`, `failed`).
- [ ] Add `outbox schedule add` and `outbox schedule list` commands.
- [ ] Add worker mode to process due jobs from local schedule storage.
- [x] Add idempotency key strategy to prevent duplicate LinkedIn posts.
- [ ] Add retry policy with capped attempts and error logging.
- [x] Add per-job publish audit log file (JSON lines) for direct publish commands.
- [ ] Extend audit logging coverage to scheduled worker attempts and retries.
- [ ] Add OS integration docs for running worker in background (`launchd`, `systemd`, Task Scheduler).
- [ ] Add dry-run mode for scheduler verification without publishing.
- [ ] Add volume checkpoint for persistence strategy:
- [ ] Stay file-first for early scale.
- [ ] Define threshold and migration trigger to SQLite (for example around thousands of published items and growing query latency).

## Phase 5 - Substack Support

- [ ] Evaluate and choose Substack integration strategy (official path or maintained community integration).
- [ ] Implement Substack adapter behind shared publisher interface.
- [ ] Add `outbox publish substack --file <path>` command path.
- [ ] Add scheduler integration for Substack with clear failure handling.
- [ ] Isolate Substack-specific dependency and fallback path in docs.

## Phase 6 - Instagram Support

- [ ] Implement Instagram adapter behind shared publisher interface.
- [ ] Add media validation requirements (image/video format and size checks).
- [ ] Add `outbox publish instagram --file <path> --media <path>` command path.
- [ ] Add scheduler integration for Instagram and independent failure handling.
- [ ] Add platform-specific preflight checks before scheduling.

## Phase 7 - Remote Control (Optional, Local-First Preserved)

- [ ] Add command-ingest interface for remote triggers (Telegram bot or simple HTTP endpoint).
- [ ] Keep execution on local machine while online service only sends commands.
- [ ] Add authenticated command queue with replay protection.
- [ ] Add status callbacks so remote client can see publish result.
- [ ] Document behavior when local machine is offline.

## Phase 8 - Dashboard (Optional)

- [ ] Expose local API from Rust worker for dashboard consumption.
- [ ] Build TypeScript dashboard for content list, schedule view, and publish actions.
- [ ] Add sync view for local file status and job status.
- [ ] Keep dashboard as interface layer; keep publish logic in Rust core.

## Phase 9 - Public Distribution (Optional, End-Stage)

- [ ] Build and test release binaries for macOS, Linux, and Windows.
- [ ] Add release packaging notes for external developers (checksums, naming, changelog).
