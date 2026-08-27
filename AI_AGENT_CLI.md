# Publo AI Agent CLI Playbook

Use this playbook when an AI agent operates Publo from terminal.

## Rules

- The configured `publo` CLI command is the only interface for Publo state and writes.
  Publo's configured workspace database is the source of truth; do not infer state from the
  current directory. Do not override `HOME`, use an alternate test workspace, inspect or
  modify the database, or bypass the CLI with filesystem operations.
- Always use JSON output from `publo` and verify results.
- For write commands that support it, always pass `--by ai`.
- Add `--ai-model` and `--ai-note` on AI-written decisions.
- Verify each write batch with `publo job list`.
- Do not move or rename short-form content files. File identity tracking for moves and renames is a future Publo capability; create new files when new content is needed.
- Name new short-form files according to `references/short-form-content-rule.md`: `linkedin-post`, `x-post`, or `x-thread` identifies the intended platform and format. Treat this as a catalog hint only; Publo remains authoritative for the selected platform, schedule, and status.
- Legacy filenames remain valid. Do not rename them merely to match the new convention.
- The only author-change signal currently available is Publo's last-modified-by flag (`ai` or `user`); do not describe it as a full audit history.
- Store and reason about publish times through Publo. The database stores UTC; `--timezone` controls dashboard display and human-facing scheduling.
- Best-effort only: do as much as possible, then report what could not be done and why.
- Do not try to enforce impossible targets (for example not enough content).
- File/path checks and duplicate protection are handled by Publo core.

## Publo State Model

These are the complete job statuses currently supported:

- `ready` = editorially selected (or needs very minor work) and eligible for scheduling
- `scheduled` = assigned to a platform and publish time
- `publishing` = publish attempt is in progress
- `published` = publish completed successfully
- `failed` = publish attempt completed unsuccessfully
- `blocked` = Publo rejected the job before scheduling or publishing; inspect its error reason
- `canceled` = a scheduled job was manually canceled; preserve the cancellation reason
- `disabled` = Publo disabled the job; inspect its reason before changing it

Lifecycle expectations:

- A ready decision can become `blocked` or `disabled` immediately when Publo validation rejects it.
- A scheduled job can be manually canceled and later scheduled again if the content remains eligible.
- A scheduled publish normally moves through `publishing` and then ends as either `published` or `failed`.
- Publo's status and reason are authoritative. Do not infer publication from filenames, timestamps, or the absence of an error in a separate file.

## Core Commands

- Mark/update decision record from file:
  - `publo job ready --file /abs/path/post.md --platform linkedin,x --at 2026-08-26T09:00:00+03:30 --timezone Asia/Tehran --by ai --ai-model gpt-5 --ai-note "why this is suitable"`
- Read jobs:
  - `publo job list --status ready --limit 500`
  - `publo job list --status blocked --limit 500`
  - `publo job list --status canceled --limit 500`
  - `publo job list --status disabled --limit 500`
  - `publo job list --status scheduled --limit 500`
  - `publo job show --id <job_id>`
- Preflight/debug a job:
  - `publo job run-debug --id <job_id>`
- Import a publication that happened before Publo tracked it:
  - `publo job import-published --file /abs/path/post.md --platform linkedin,x --published-at 2026-08-20T10:30:00+03:30 --timezone Asia/Tehran --by ai --ai-model gpt-5 --ai-note "how the publication was verified"`
- Schedule a job by id:
  - `publo job schedule --id <job_id> --platform linkedin --at 2026-08-26T09:00:00+03:30 --timezone Asia/Tehran --by ai --ai-model gpt-5 --ai-note "schedule decision"`
- Schedule directly from file (upsert by file identity + platform):
  - `publo job add-schedule --file /abs/path/post.md --platform x --at 2026-08-26T12:00:00+03:30 --timezone Asia/Tehran --by ai --ai-model gpt-5 --ai-note "direct schedule"`
- Unschedule/cancel/remove:
  - `publo job unschedule --id <job_id> --reason "reschedule requested"`
  - `publo job cancel --id <job_id> --reason "manual stop"`
  - `publo job unready --id <job_id>`

## Scenario A: Make Files Ready From Catalog

1. Enumerate candidate markdown files from catalog roots.
2. For each file, decide:
   - platforms (`linkedin`, `x`, or both),
   - optional suggested publish time (`--at` + `--timezone`).
   - For newly named assets, use the `linkedin-post`, `x-post`, or `x-thread` filename token as the intended format. Confirm the actual platform through the Publo command rather than inferring job state from the filename.
3. Run `publo job ready ... --by ai ...` for each file.
4. Verify:
   - `publo job list --status ready --limit 500`
   - `publo job list --status blocked --limit 500`
5. Report:
   - files processed,
   - files made ready,
   - files skipped and reasons.

## Scenario B: Read/Adjust Existing Decision Queue Jobs

1. Pull queue states:
   - `ready`, `blocked`, `canceled`, `disabled`.
2. Inspect per item:
   - `publo job show --id ...`
   - `publo job run-debug --id ...` when needed.
3. Adjust decision by re-running `publo job ready --file ... --by ai ...` (updates existing decision record for that file identity).
4. Verify with `publo job list` for affected statuses.

## Scenario C: Build a Weekly Schedule (Best Effort)

Example user request:
- “Next week: 1 LinkedIn/day, 3 short X posts, 2 threads.”

Process:

1. Build candidate pool from decision queue jobs (`ready/blocked/canceled/disabled`) and run debug preflight where needed.
2. Select best candidates matching user guidance.
3. Schedule with:
   - `publo job schedule --id ... --platform ... --at ... --timezone ... --by ai ...`
   - or `publo job add-schedule --file ... --platform ... --at ... --timezone ... --by ai ...`
4. Verify:
   - `publo job list --status scheduled --limit 500`
   - `publo job list --status blocked --limit 500`
5. Report:
   - what was scheduled,
   - what failed and exact error message,
   - what could not be satisfied due to content/validation limits.

## Current Capability Notes

- Platforms currently supported in job CLI: `linkedin`, `x`.
- X thread publish mode is not implemented yet in core publish flow.
- If user asks for threads:
  - do not fake it,
  - report unsupported gap clearly,
  - optionally propose X single-post fallback.

## Scenario D: Import Historical Publications

Use this only when the user asks to record content that was already published outside
Publo.

1. Confirm the exact file and platform from user-provided evidence. Do not infer publication
   from a filename or assume that content was published.
2. Include `--published-at` and `--timezone` only when the publication time is known.
3. Always use `--by ai`, `--ai-note`, and `--ai-model`.
4. Run one insert-only import command for all confirmed platforms.
5. If Publo reports an existing job conflict, do not modify or remove that job. Report the
   conflict to the user.
6. Verify with `publo job list --status published --limit 500` and `publo job show --id ...`.

Historical imports create no provider attempt. They must appear as imported publication
history rather than as content published by Publo.
