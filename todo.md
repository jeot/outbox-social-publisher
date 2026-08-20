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
- [ ] Integrate X into scheduler flow and retry/idempotency behavior.

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

## Phase 6 - Minimal Local GUI (Catalog + Ready + Scheduling)

Add a minimal local GUI to validate the real scheduling workflow before implementing worker publishing.

- [x] Add minimal catalog page to list `.md` files from configured folders.
- [x] Add collapsible multi-root file tree with persistent panel state (open/closed + width).
- [ ] Add file detail preview (content/media refs/parsed publish text).
- [ ] Add Ready page/list showing `ready` jobs.
- [ ] Add actions from GUI: ready/unready, assign/remove platform, schedule/unschedule/cancel.
- [ ] Add schedule presets:
- [ ] today/tomorrow/next-week at 9:00 / 12:00 / 16:00 / 19:00.
- [ ] +5m / +30m / +1h / +3h.
- [ ] Wire GUI actions to local backend APIs that reuse core job logic.
- [ ] Refresh status immediately after actions.

## Phase 7 - Publishing Core (Worker + Attempts)

Implement scheduled execution and attempt tracking on top of existing job lifecycle.

- [ ] Add `publo worker run` long-running process for due jobs.
- [ ] Add atomic claim flow (`scheduled` -> `publishing`) to avoid duplicate execution.
- [ ] Execute publish via existing platform adapters.
- [ ] Write `publish_attempts` rows for every attempt.
- [ ] Update job status (`published`/`failed`) with reason/error snapshots.
- [ ] Add retry policy with capped attempts and backoff.
- [ ] Add dry-run mode for worker verification without posting.
- [ ] Extend audit logging coverage to scheduled worker attempts and retries.
- [ ] Add OS integration docs for background service mode (`launchd`, `systemd`, Task Scheduler).

## Phase 8 - Advanced Local GUI (List + Calendar + Actions)

Expand GUI after worker behavior is available.

- [ ] Add richer schedule views (list/calendar).
- [ ] Add job timeline with attempts/errors and retry actions.
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
