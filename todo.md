# Publo Roadmap

This roadmap keeps local files as the source of truth and builds operational layers on top in small, testable phases.

## Phase 1 - Foundation (Local CLI + LinkedIn MVP)

Build a reliable local-first CLI with clear JSON contracts, auth basics, and production-safe error handling.

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
- [x] Implement `publo publish linkedin --file <path>` command.
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
- [x] Add publish CLI safety gate (`--pass`) with config-backed password for external publish commands.

## Phase 2 - X Platform Support

Bring X to feature parity with strong auth UX, validation, and publish safety.

- [x] Implement X platform adapter behind shared publisher interface.
- [x] Add guided X OAuth login (local callback server + browser flow) and token storage.
- [x] Add `publo publish x --file <path>` command path.
- [x] Add shared duplicate guard + JSONL publish logging parity for X.
- [x] Add X preflight validation for weighted-length and self-serve cashtag rule.
- [x] Add X bypass flags: `--allow-cashtag`, `--allow-length`, `--allow-duplicate`, `--force`.
- [x] Improve X API error suggestions (credits depleted, cashtag-limit, generic 403 with local hints).
- [x] Add signature support (global + per-platform config, plus CLI overrides) for LinkedIn and X.
- [x] Add `file_sha256` + `text_sha256` tracking in output and publish logs.
- [x] Document and enforce `media.write` scope requirement for X image posts.
- [x] Add X token tooling parity commands (`auth x token-status`, `auth x token-refresh`).
- [x] Add X manual auth exchange fallback (`auth x exchange`).
- [x] Integrate X into scheduler and supervised worker execution.
- [ ] Add worker retry policy without weakening duplicate/idempotency safeguards.

## Phase 3 - Media (Images)

Support file-driven media publishing while preserving Obsidian-friendly authoring.

- [x] Define Obsidian-note parsing contract:
- [x] Publish text from section after last `---`.
- [x] Discover `![[...]]` media embeds from whole file (ordered).
- [x] Strip embed placeholders from publish text before send.
- [x] Resolve media from note folder first, then `config.toml` media lookup paths.
- [x] Validate media extension (`.png`, `.jpg`, `.jpeg`) and block on invalid/missing files.
- [x] Add LinkedIn single-image upload flow (`rest/images initializeUpload` + upload + `rest/posts` media content).
- [x] Add LinkedIn multi-image publish support (MultiImage API, 2-20 images).
- [x] Add X image upload/publish support (1-4 images via media upload + `media_ids`).

## Phase 4 - State Model Foundation (SQLite + Lifecycle)

Create the persistent operational model for catalog, scheduling, attempts, and status transitions while keeping content in local files.

- [x] Introduce SQLite storage for operational state (no content lock-in).
- [x] Define schema for jobs, publish attempts, and sync metadata foundation.
- [x] Define canonical status lifecycle (`ready`, `scheduled`, `publishing`, `published`, `failed`, `blocked`, `canceled`, `disabled`).
- [x] Define status/platform constraint policy:
- [x] `ready` may have platform or be platform-less.
- [x] all non-`ready` statuses require a platform.
- [x] Add migration/versioning strategy for schema evolution (auto-run pending migrations at startup).
- [x] Record immutable attempt history for audit and debugging.
- [x] Keep file path + hashes (`file_sha256`, `text_sha256`, `fingerprint`) linked to jobs.
- [x] Add job metadata for operator attribution (`user|ai`), notes, and tags.

## Phase 5 - Scheduling Core (CLI Jobs)

Implement the core scheduling experience in CLI and validate lifecycle/state behavior before worker execution.

- [x] Define schedule input contract (platform(s), time, timezone, file reference, optional note).
- [x] Add job lifecycle commands (`publo job ready|unready|schedule|unschedule|add-schedule|list|show|cancel|run-debug`).
- [x] Add schedule-time preflight behavior (auto block with reason on invalid/auth/content checks).
- [x] Enforce lifecycle behavior in commands (no cancel on `ready`; assign platform at scheduling when missing).
- [x] Add workspace bootstrap + switching flow (`publo init`, `publo workspace switch`).
- [x] Separate workspace identity model (`workspace_id`) from display name (`workspace.display_name`).
- [x] Add getting-started + platform setup docs for first-time users.
- [x] Enforce uniqueness and upsert behavior for decision/schedule writes to prevent duplicate logical jobs.
- [x] Separate decision platform intent from tags (`selected_platforms` field).
- [x] Allow marking broken files as ready; keep strict publishability checks at scheduling/publish time.

## Phase 6 - Minimal Local GUI (Catalog + Ready + Scheduling)

Add a minimal local GUI to validate the real scheduling workflow before implementing worker publishing.

- [x] Add minimal catalog page to list `.md` files from configured folders.
- [x] Add collapsible multi-root file tree with persistent panel state (open/closed + width).
- [x] Add file detail preview (content/media refs/parsed publish text).
- [x] Persist selected file on refresh.
- [x] Show ready counts on folder/root nodes.
- [x] Show AI-ready file badge variant (`Ready` + AI icon).
- [x] Return file-level jobs array in catalog file API (`jobs: []` for selected file).
- [x] Add Decision Queue showing `ready`, `blocked`, `canceled`, `disabled`, and `failed` jobs.
- [x] Add GUI actions for ready/unready, platform selection, schedule/reschedule, cancel, and remove.
- [x] Add schedule presets:
- [x] today/tomorrow/next-week at 9:00 / 12:00 / 16:00 / 19:00.
- [x] +5m / +30m / +1h / +3h.
- [x] Wire GUI actions to local backend APIs that reuse core job logic.
- [x] Refresh status immediately after actions.
- [x] Add production UI serving mode from backend (`publo serve` serves `web/dist` + API).

## Phase 7 - Publishing Core (Worker + Attempts)

Implement scheduled execution and attempt tracking on top of existing job lifecycle.

- [x] Add one-shot dry-run worker (`publo worker run --dry-run --once`) that reads and preflights due jobs without state changes or network publishing.
- [x] Add password-gated supervised live worker (`publo worker run --live --once`) that processes at most one due job.
- [x] Add atomic claim flow (`scheduled` -> `publishing`) to avoid duplicate execution.
- [x] Execute worker publishes through existing LinkedIn and X platform adapters.
- [x] Write `publish_attempts` rows for worker attempts.
- [x] Update job status (`published`/`failed`/`blocked`) with error snapshots.
- [x] Reconcile claims stranded in `publishing` for five minutes without automatically retrying an uncertain provider request.
- [x] Add deterministic fake-provider tests for claim, success, preflight block, provider failure, crash, and workspace isolation behavior.
- [ ] Complete a seven-day supervised live pilot with real scheduled content and review every result.
- [ ] Define pilot exit criteria from observed runs: no duplicates, correct oldest-due ordering, understandable failures, and reliable attempt history.
- [ ] Add `publo worker run --live` long-running loop with a one-minute due-job poll interval.
- [ ] Add graceful shutdown and guarantee only one active claim is processed at a time.
- [ ] Add loop-level resilience for database/provider errors without terminating the worker or bypassing claim safety.
- [ ] Add retry policy with capped attempts and backoff.
- [ ] Extend audit logging coverage to scheduled worker attempts and retries.
- [ ] Add OS integration docs for background service mode (`launchd`, `systemd`, Task Scheduler).

## Phase 8 - Advanced Local GUI (List + Calendar + Actions)

Expand GUI after worker behavior is available.

- [ ] Add richer schedule views (list/calendar).
- [x] Add lifecycle views for publishing, failed, and published jobs.
- [x] Add chronological publish-attempt details in the preview sidebar.
- [ ] Add explicit retry/reschedule actions for failed jobs.
- [ ] Add filters by platform/status/date.
- [ ] Keep GUI as interface layer; keep publish/schedule logic in Rust core.

## Phase 9 - Remote Worker + Sync (Optional)

Allow always-on publishing even when laptop is offline by separating authoring location from worker runtime.

- [ ] Define local-vs-remote worker execution model with same core semantics.
- [ ] Define sync strategy for content + schedule state (tool-agnostic).
- [ ] Add sync conflict policy and resolution rules.
- [ ] Add remote secrets/auth management model.
- [ ] Add result/status sync back to local machine and GUI.
- [ ] Document offline behavior and recovery guarantees.

## Phase 10 - Platform Expansion (Substack, Instagram)

Expand platform coverage using the same publish pipeline and scheduler architecture.

- [ ] Evaluate and choose Substack integration strategy (official path or maintained community integration).
- [ ] Implement Substack adapter behind shared publisher interface.
- [ ] Add `publo publish substack --file <path>` command path.
- [ ] Add scheduler integration for Substack with clear failure handling.
- [ ] Isolate Substack-specific dependency and fallback path in docs.
- [ ] Implement Instagram adapter behind shared publisher interface.
- [ ] Add media validation requirements (image/video format and size checks).
- [ ] Add `publo publish instagram --file <path> --media <path>` command path.
- [ ] Add scheduler integration for Instagram and independent failure handling.
- [ ] Add platform-specific preflight checks before scheduling.

## Phase 11 - Public Distribution (Optional, End-Stage)

Package Publo for external developers with reproducible builds and release hygiene.

- [ ] Build and test release binaries for macOS, Linux, and Windows.
- [ ] Add release packaging notes for external developers (checksums, naming, changelog).

## Backlog - Convenience UX

- [ ] Add one-liner multi-platform command:
  - Example: `publo "Failure is part of building things."`
  - Behavior: publish same text to all enabled/configured platforms.
  - Output: per-platform success/failure JSON summary.
