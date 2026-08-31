# Publo

Your personal publishing pipeline.  
Your content stays local. Publo takes it from there.

Vision and direction: [vision.md](vision.md)
Quick setup guide: [GETTING_STARTED.md](GETTING_STARTED.md)
Platform setup guides:

- [docs/platforms/linkedin-setup.md](docs/platforms/linkedin-setup.md)
- [docs/platforms/x-setup.md](docs/platforms/x-setup.md)
- [docs/platforms/substack-setup.md](docs/platforms/substack-setup.md)

## Current status

- LinkedIn: text + image + multi-image publishing
- X: text + image publishing (up to 4 images)
- Substack Notes: live-verified text and image publishing
- Guided OAuth login for both platforms
- Manual OAuth exchange fallback for both platforms
- Token status/refresh commands for both platforms
- Local-first parsing for Obsidian notes (`---` split + `![[...]]`)
- Duplicate guard + JSONL publish log
- SQLite scheduling lifecycle with immutable publish attempts
- Supervised one-job live worker for LinkedIn, X, and Substack Notes
- Local GUI for catalog, decision queue, scheduled, publishing, failed, and published states

## Feature matrix

| Capability | LinkedIn | X | Substack Notes |
|---|---|---|---|
| Text post | ✅ | ✅ | ✅ live-verified |
| Single image | ✅ | ✅ | ✅ live-verified |
| Multi-image | ✅ (2-20) | ✅ (1-4) | 🧪 contract-tested |
| Guided OAuth callback login | ✅ | ✅ | N/A |
| Manual OAuth exchange | ✅ | ✅ | N/A |
| Auth/session status | ✅ | ✅ | ✅ |
| Token refresh | ✅ | ✅ | Manual session replacement |
| Auto refresh during publish on 401 | ✅ | ✅ | N/A |
| Debug no-send mode (`--debug`) | ✅ | ✅ | ✅ |
| Scheduled one-shot worker | ✅ | ✅ | ✅ implemented; live worker check pending |

## Prerequisites

- Rust toolchain (`cargo`)
- A LinkedIn/X developer app and/or an authenticated Substack browser session

If you are running directly from source without installing the binary, prepend commands with `cargo run --`  
(example: `cargo run -- auth x login`).

## Quick start

1. Initialize or inspect the active workspace, then edit the reported `env_path`:

```bash
publo init
publo paths
```

2. Configure readable JSON + local media lookup in `config.toml`:

```toml
[output]
pretty_json = true

[media]
lookup_paths = ["/absolute/path/to/media-assets"]
```

3. Run auth:

```bash
publo auth linkedin login
publo auth x login
publo auth substack whoami
```

4. Validate without posting:

```bash
publo publish linkedin --file ./post.md --debug
publo publish x --file ./post.md --debug
publo publish substack --file ./note.md --debug
```

5. Publish:

```bash
publo publish linkedin --file ./post.md --pass <publish-pass>
publo publish x --file ./post.md --pass <publish-pass>
publo publish substack --file ./note.md --pass <publish-pass>
```

## Worker pilot

Inspect every due scheduled job without changing the database or sending a post:

```bash
publo worker run --dry-run --once
```

The dry run reads all due jobs, performs current file/media/auth preflight checks, and
returns per-job JSON showing whether each item would publish.

Publish at most one due job under direct supervision:

```bash
publo worker run --live --once --pass <publish-pass>
```

`--live --once` claims and processes only the oldest due job. If multiple jobs are due,
repeat the supervised command once per intended publication. Confirm the queue between
runs with `--dry-run --once`; stop when `due_count` is `0`.

### Seven-day supervised pilot

Before enabling a continuous worker loop:

1. Keep the first week small: one or two intentional publishing windows per day.
2. Run `--dry-run --once` immediately before every live publishing session.
3. Review each due item's file, platform, media, and local schedule time in the GUI.
4. Run one live command, then confirm the result under Published or Decision Queue.
5. If more jobs are due, repeat from the dry-run check rather than publishing a batch blindly.
6. Do not move or rename a scheduled file after the final dry run.
7. Stop the pilot on any unexpected content, platform, duplicate, provider error, or stranded
   `publishing` state and investigate before the next live run.

There is no automatic retry. An interrupted claim is considered unsafe to retry
automatically because the provider may have accepted the post before the worker crashed.
Claims left in `publishing` for five minutes are reconciled on a later live worker run and
made visible for human review.

## Recommended local workflow (Obsidian-friendly)

- Keep your content in local markdown files.
- Optional metadata/properties above separators.
- Publishable content starts after the **last** `---`.
- Image embeds `![[...]]` can appear anywhere in the file.
- Publo resolves image files from:
  1) note folder  
  2) `[media].lookup_paths` in `config.toml`

This keeps content local and avoids copy/paste into SaaS editors.

## `config.toml` options

```toml
[output]
pretty_json = true

[timeouts]
connect_seconds = 10
request_seconds = 30

[signature]
enabled = true
text = "\n[Sent from publo]"

[platform.linkedin.signature]
enabled = true
# text = "\n[Sent from publo]"

[platform.x.signature]
enabled = false
# text = "\n[Sent from publo]"

[platform.substack.signature]
enabled = false
# text = "\n[Sent from publo]"

[media]
lookup_paths = ["/absolute/path/to/media-assets"]

[security]
publish_cli_password = "shk"
```

`publish_cli_password` is required by real CLI publish commands via `--pass` (not required for `--debug`).

Signature precedence:

- CLI flag (`--add-signature` / `--no-signature`)
- platform signature config
- global signature config
- default off

## `.env` values

```dotenv
# LinkedIn app credentials/settings
LINKEDIN_CLIENT_ID=
LINKEDIN_CLIENT_SECRET=
LINKEDIN_REDIRECT_URI=http://localhost:8788/callback
LINKEDIN_SCOPES='w_member_social openid profile'
LINKEDIN_API_VERSION=202607

# LinkedIn runtime auth values
LINKEDIN_ACCESS_TOKEN=
LINKEDIN_REFRESH_TOKEN=
LINKEDIN_ACCESS_TOKEN_EXPIRES_IN=
LINKEDIN_REFRESH_TOKEN_EXPIRES_IN=
LINKEDIN_AUTHOR_URN=urn:li:person:

# X app credentials/settings
X_CLIENT_ID=
X_CLIENT_SECRET=
X_REDIRECT_URI=http://127.0.0.1:8789/callback
X_SCOPES='tweet.read tweet.write users.read media.write offline.access'

# X runtime auth values
X_ACCESS_TOKEN=
X_REFRESH_TOKEN=
X_ACCESS_TOKEN_EXPIRES_IN=
X_TOKEN_TYPE=

# Substack Notes browser-session authentication
SUBSTACK_SESSION_TOKEN=
SUBSTACK_PUBLICATION_URL=https://yourname.substack.com
```

## LinkedIn setup

1. Create/select app in LinkedIn Developer Portal.
2. Add redirect URL exactly matching `LINKEDIN_REDIRECT_URI`.
3. Enable products:
   - Share on LinkedIn
   - Sign In with LinkedIn using OpenID Connect
4. Set `LINKEDIN_API_VERSION` to active version from your app portal.

Auth commands:

```bash
publo auth linkedin guide
publo auth linkedin login
publo auth linkedin exchange --code <code> --state <state>
publo auth linkedin whoami
publo auth linkedin token-status
publo auth linkedin token-refresh
```

Publish:

```bash
publo publish linkedin --file ./post.md --pass <publish-pass>
```

Options:

- `--pass <publish-pass>` (required for real publish, optional with `--debug`)
- `--allow-duplicate`
- `--debug`
- `--add-signature`
- `--no-signature`

Image limits:

- single image: supported
- multi-image: supported, max 20
- allowed extensions: `.png`, `.jpg`, `.jpeg`

## Substack Notes setup

Substack Notes uses a manually copied browser session instead of OAuth. Configure
`SUBSTACK_SESSION_TOKEN` with only the value of the `substack.sid` cookie and set
`SUBSTACK_PUBLICATION_URL`. Treat the session value like a password.

Auth commands:

```bash
publo auth substack guide
publo auth substack session-status
publo auth substack whoami
```

Publish Notes only—Substack articles are intentionally unsupported:

```bash
publo publish substack --file ./note.md --debug
publo publish substack --file ./note.md --pass <publish-pass>
```

Options:

- `--pass <publish-pass>` (required for real publish, optional with `--debug`)
- `--allow-duplicate`
- `--debug`
- `--add-signature`
- `--no-signature`

Text-only and image Notes use the shared Obsidian parsing and media-resolution rules.
Text-only publishing was verified against a real Substack feed on August 31, 2026.
Image publishing was also verified against a real Substack feed on August 31, 2026.
See [the setup guide](docs/platforms/substack-setup.md) and the
[pinned unofficial API reference](docs/platforms/substack-unofficial-api-reference.md).

Substack scheduling and `publo worker run` use `publish_mode = "note"` to remain
separate from possible future article support. Migration `0002` removes the original
closed publish-mode constraint while preserving jobs and attempts, so future modes do
not require schema changes. The initial migration remains immutable. Complete a
supervised dry-run and one live worker publication before treating the worker path as
operationally verified. A scheduled text Note was successfully published with
`publo worker run --live --once` on August 31, 2026; dry-run and scheduled image-Note
verification remain.

## X setup

### Billing heads-up

X API is pay-per-usage. Add credits in **console.x.com → Billing → Credits**.

If credits are depleted, you will get `402` with `credits depleted`.

### App settings

In X OAuth 2.0 user auth settings:

- Type: `Native App` (`Public client`)
- Permissions: `Read and write`
- Callback URL: `http://127.0.0.1:8789/callback`
- Scopes include `media.write` for image posting

Auth commands:

```bash
publo auth x login
publo auth x exchange --code <code> --state <state>
publo auth x token-status
publo auth x token-refresh
```

If you change scopes, run `auth x login` again to issue a new token.

Publish:

```bash
publo publish x --file ./post.md --pass <publish-pass>
```

Options:

- `--pass <publish-pass>` (required for real publish, optional with `--debug`)
- `--allow-duplicate`
- `--allow-cashtag`
- `--allow-length`
- `--force`
- `--debug`
- `--add-signature`
- `--no-signature`

Image limits:

- single/multi-image supported
- max 4 images
- allowed extensions: `.png`, `.jpg`, `.jpeg`

## Obsidian parsing rules

- Publish text = content after the last `---`.
- If only one separator exists, text is after it.
- If no separator exists, full file is text.
- `![[...]]` placeholders are removed from outgoing text.
- Final text is trimmed.

## Debug mode (safe, no posting)

Use `--debug` on publish commands.

It validates:

- parsing + final text
- media resolution
- extension rules
- duplicate guard
- platform preflight checks

It returns payload preview JSON and does not send API publish requests.

## Output and logs

- All command output is JSON.
- Publish success includes:
  - `post_id`, `post_url`, `request_id`, `published_at`
  - `fingerprint`, `file_sha256`, `text_sha256`
  - `token_refreshed`
- Local log: `.publo/publish-log.jsonl`

## Exit codes

- `0`: success
- `2`: validation error
- `3`: missing auth/config
- `4`: local IO error
- `5`: HTTP/API error
- `6`: duplicate publish blocked
