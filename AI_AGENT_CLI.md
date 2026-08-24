# Publo AI Agent CLI Playbook

Use this playbook when an AI agent operates Publo from terminal.

## Rules

- Always use JSON output from `publo` and verify results.
- For write commands that support it, always pass `--by ai`.
- Add `--ai-model` and `--ai-note` on AI-written decisions.
- Verify each write batch with `publo job list`.
- Best-effort only: do as much as possible, then report what could not be done and why.
- Do not try to enforce impossible targets (for example not enough content).
- File/path checks and duplicate protection are handled by Publo core.

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

## Optional Debug-First Checks On Raw Files

Before `job ready` or scheduling, agent may run publish debug checks without password (debug does not publish):

- `publo publish linkedin --file /abs/path/post.md --debug`
- `publo publish x --file /abs/path/post.md --debug`

Use this when you want early feedback on media/text/auth issues before queue actions.
