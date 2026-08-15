# Outbox

Local-first CLI for publishing content from files.

Current status: LinkedIn text publishing MVP in progress.
X text publishing is available via guided OAuth 2.0 login.

## Prerequisites

- Rust toolchain (`cargo`)
- A LinkedIn account
- A LinkedIn developer app (steps below)

## Quick Start

1. Copy environment template:

```bash
cp .env.example .env
```

2. Optional: choose readable JSON output:

Set in `config.toml`:

```toml
[output]
pretty_json = true
```

Optional signature config (global + per-platform override):

```toml
[signature]
enabled = true
text = "\n[Sent from outbox, by shk]"

[platform.linkedin.signature]
enabled = true
# text = "\n[Sent from outbox, by shk]"

[platform.x.signature]
enabled = false
# text = "\n[Sent from outbox, by shk]"
```

3. Follow LinkedIn app setup and OAuth steps in the next sections.

4. Publish from a local text file:

```bash
cargo run -- publish linkedin --file ./your-post.md
cargo run -- publish x --file ./your-post.md
```

## LinkedIn Developer App Setup

1. Open LinkedIn Developer Portal and create (or select) an app.
2. In app `Auth`, copy:
   - `Client ID`
   - `Client Secret`
3. In app `Auth`, add an OAuth redirect URL.
   - Example used by this repo: `http://localhost:8788/callback`
   - It must exactly match `LINKEDIN_REDIRECT_URI` in your `.env`.
4. In app `Products`, enable:
   - `Share on LinkedIn`
   - `Sign In with LinkedIn using OpenID Connect`
5. For `Share on LinkedIn`, use the product version shown in your app portal and place it in `LINKEDIN_API_VERSION`.

Useful references:

- [Authorization Code Flow](https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow)
- [LinkedIn Marketing Quick Start](https://learn.microsoft.com/en-us/linkedin/marketing/quick-start?view=li-lms-2026-07)
- [Increasing Access](https://learn.microsoft.com/en-us/linkedin/marketing/increasing-access?view=li-lms-2026-06)
- [Posts API](https://learn.microsoft.com/en-us/linkedin/marketing/community-management/shares/posts-api?view=li-lms-2026-06)

## `.env` Values

Set these values in `.env`:

```dotenv
# LinkedIn app credentials/settings
LINKEDIN_CLIENT_ID=
LINKEDIN_CLIENT_SECRET=
LINKEDIN_REDIRECT_URI=http://localhost:8788/callback
LINKEDIN_SCOPES='w_member_social openid profile'

# LinkedIn runtime auth values
LINKEDIN_ACCESS_TOKEN=
LINKEDIN_REFRESH_TOKEN=
LINKEDIN_ACCESS_TOKEN_EXPIRES_IN=
LINKEDIN_REFRESH_TOKEN_EXPIRES_IN=
LINKEDIN_AUTHOR_URN=urn:li:person:
LINKEDIN_API_VERSION=202607

# X app credentials/settings
X_CLIENT_ID=
X_CLIENT_SECRET=
X_REDIRECT_URI=http://127.0.0.1:8789/callback
X_SCOPES='tweet.read tweet.write users.read offline.access'

# X runtime auth values
X_ACCESS_TOKEN=
X_REFRESH_TOKEN=
X_ACCESS_TOKEN_EXPIRES_IN=
```

## X Quick Start (Guided OAuth 2.0)

### Billing heads-up (important)

X API posting uses **pay-per-usage** pricing. Before testing publish calls, verify your project has available credits.

- Open [X Developer Console](https://console.x.com/)
- Go to your Project/App billing
- Add credits in **Billing → Credits**

If credits are depleted, publish returns an explicit `402` error:

```json
{
  "ok": false,
  "error_type": "http_error",
  "message": "X API returned 402",
  "http_status": 402,
  "api_error": {
    "detail": "credits depleted",
    "status": 402,
    "title": "Payment Required",
    "type": "https://api.x.com/2/problems/credits-depleted"
  },
  "retryable": false,
  "suggestion": "X API credits are depleted for this app/project. Enable billing or upgrade access in X Developer Portal, then retry publish.",
  "command": null
}
```

URL-cost behavior observed during testing:

- Posts containing explicit `https://...` or `http://...` were billed as **with URL** requests.
- Bare domains like `google.com` and `shamimkeshani.ir` were not billed at the higher URL rate in our tests.

Because provider billing behavior can change, verify current rates and categories in X pricing docs and your usage dashboard.

### 1) Create/Configure app in X Console

Open [X Developer Console](https://console.x.com/) and create/select your Project + App.

In **User authentication settings** (OAuth 2.0), use:

- **Type of App**: `Native App` (`Public client`)
- **App permissions**: `Read and write`
- **Request email from users**: `Off` (not needed for posting)
- **Callback / Redirect URL**: exactly `http://127.0.0.1:8789/callback`
- **Website URL**: your repo URL (for example `https://github.com/jeot/outbox-social-publisher`)

Save settings.

### 2) Copy OAuth 2.0 keys and set `.env`

From X Console, copy:

- `X_CLIENT_ID` (from OAuth 2.0 Keys)
- `X_CLIENT_SECRET` (if shown for your app type/settings)

Set:

- `X_REDIRECT_URI=http://127.0.0.1:8789/callback`
- `X_SCOPES='tweet.read tweet.write users.read offline.access'`

### 3) Understand key naming (important)

X Console may also show:

- `X_CONSUMER_KEY`
- `X_SECRET_KEY`
- `X_BEARER_TOKEN`

These are app-level credentials from API key/bearer models and are **not** the `X_ACCESS_TOKEN` used by this CLI OAuth 2.0 user flow.

For this repository flow, use:

- `X_CLIENT_ID`
- `X_CLIENT_SECRET` (if present)
- `X_REDIRECT_URI`
- `X_SCOPES`

Then `outbox auth x login` obtains and saves:

- `X_ACCESS_TOKEN`
- optional `X_REFRESH_TOKEN`

You can remove `X_CONSUMER_KEY`, `X_SECRET_KEY`, and `X_BEARER_TOKEN` from `.env` if you are not implementing OAuth 1.0a/app-only endpoints.

### 4) Run guided login

```bash
cargo run -- auth x login
```

Behavior:

- starts localhost callback server
- opens browser and prints auth URL
- waits for callback (Ctrl-C cancels)
- exchanges code for token and saves `.env`
- shows success/failure in both browser and terminal

### 5) Publish

```bash
cargo run -- publish x --file ./post.md
```

Expected success JSON:

```json
{
  "ok": true,
  "platform": "x",
  "post_id": "<tweet-id>",
  "post_url": "https://x.com/i/web/status/<tweet-id>",
  "request_id": "<x-request-id-or-null>",
  "published_at": "2026-08-15T12:34:56Z"
}
```

## Auth Commands (Non-Interactive)

1. Check what is missing:

```bash
cargo run -- auth linkedin guide
```

Expected JSON (first run):

```json
{
  "mode": "auth_guide",
  "next": {
    "command": "outbox auth linkedin guide",
    "message": "LinkedIn app settings are incomplete.",
    "required_env": [
      "LINKEDIN_CLIENT_ID",
      "LINKEDIN_REDIRECT_URI",
      "LINKEDIN_CLIENT_SECRET"
    ]
  },
  "ok": true,
  "platform": "linkedin"
}
```

2. Start OAuth login (guided flow):

```bash
cargo run -- auth linkedin login
```

What it does:

- starts a localhost callback server from `LINKEDIN_REDIRECT_URI`
- opens browser (and also prints the auth URL)
- waits for callback (Ctrl-C cancels)
- exchanges code for tokens automatically
- resolves `/v2/userinfo` automatically and saves `LINKEDIN_AUTHOR_URN`
- shows matched status in browser + terminal JSON

Expected JSON:

```json
{
  "browser_opened": true,
  "access_token_saved": true,
  "access_token_expires_in": 5183999,
  "author_urn": "urn:li:person:<id>",
  "author_urn_saved_to_env": true,
  "mode": "auth_linkedin_login",
  "name": "<member name>",
  "next": {
    "command": "outbox publish linkedin --file <path>",
    "message": "LinkedIn auth completed. You can publish now."
  },
  "ok": true,
  "platform": "linkedin",
  "refresh_token_saved": false
}
```

3. Optional manual fallback (only if callback flow is unavailable):

```bash
cargo run -- auth linkedin exchange --code <copied-code> --state <state-from-login>
```

This exchanges and stores token values in your local `.env`. Then run:

```bash
cargo run -- auth linkedin whoami
```

Expected JSON:

```json
{
  "access_token_expires_in": 5183999,
  "mode": "auth_exchange",
  "next": {
    "command": "outbox auth linkedin whoami",
    "message": "Resolve and save LINKEDIN_AUTHOR_URN before publishing."
  },
  "ok": true,
  "platform": "linkedin",
  "refresh_token_saved": false,
  "token_saved_to_env": true
}
```

4. Check current token availability/state:

```bash
cargo run -- auth linkedin token-status
```

Expected JSON:

```json
{
  "ok": true,
  "platform": "linkedin",
  "mode": "auth_token_status",
  "access_token_present": true,
  "refresh_token_present": false,
  "author_urn_present": true,
  "access_token_expires_in": "5183999",
  "refresh_token_expires_in": null
}
```

5. Force a refresh attempt (for explicit verification):

```bash
cargo run -- auth linkedin token-refresh
```

Expected success JSON:

```json
{
  "ok": true,
  "platform": "linkedin",
  "mode": "auth_token_refresh",
  "token_refreshed": true,
  "access_token_expires_in": 5183999,
  "refresh_token_saved": true,
  "next": {
    "message": "Token refresh completed.",
    "command": "outbox auth linkedin token-status"
  }
}
```

## First Publish

1. Create a text file with the post content.
2. Run:

```bash
cargo run -- publish linkedin --file ./post.md
```

Expected success JSON:

```json
{
  "ok": true,
  "platform": "linkedin",
  "post_id": "<linkedin-post-id-or-restli-id>",
  "post_url": null,
  "request_id": "<restli-id>",
  "published_at": "2026-08-15T12:34:56Z",
  "token_refreshed": false
}
```

Historical first live publish output:

```json
{
  "ok": true,
  "platform": "linkedin",
  "post_id": "urn:li:share:7494283356650733568",
  "post_url": null,
  "published_at": "2026-08-15T06:46:29.236629+00:00",
  "request_id": "urn:li:share:7494283356650733568"
}
```

Recent publish output with link and content hash:

```json
{
  "content_sha256": "af2ba976553469d7f44589354dfba2f8628a29809c8b270235484c779d541024",
  "duplicate_guard": "checked",
  "fingerprint": "1d9469a2e2b19154b6296d7cd4b06f3e34f5c7c9193e07fb470658a0064b1acf",
  "ok": true,
  "platform": "linkedin",
  "post_id": "urn:li:share:7494294095167774720",
  "post_url": "https://www.linkedin.com/feed/update/urn:li:share:7494294095167774720/",
  "published_at": "2026-08-15T07:29:09.466218+00:00",
  "request_id": "urn:li:share:7494294095167774720",
  "token_refreshed": false
}
```

Direct post URL format from returned `post_id`:

```text
https://www.linkedin.com/feed/update/<post_id>/
```

Example:

```text
https://www.linkedin.com/feed/update/urn:li:share:7494283356650733568/
```

If access token is expired and refresh token is available, publish automatically:

- refreshes token once
- retries publish once
- updates `.env` token values
- returns `token_refreshed: true` on success after refresh

Duplicate protection:

- publishes are fingerprinted by `platform + author_urn + normalized content`
- same fingerprint is blocked by default to prevent accidental duplicate posts
- use `--allow-duplicate` only when you intentionally want to repost

Example override:

```bash
cargo run -- publish linkedin --file ./post.md --allow-duplicate
```

Signature behavior:

- Precedence: CLI flag > platform config > global config > off
- CLI flags (both LinkedIn and X):
  - `--add-signature` force add signature for this run
  - `--no-signature` force disable signature for this run
- If signature is enabled but no text is configured, publish returns validation error.

Examples:

```bash
cargo run -- publish linkedin --file ./post.md --add-signature
cargo run -- publish linkedin --file ./post.md --no-signature
cargo run -- publish x --file ./post.md --add-signature
```

Local publish history is stored in `.outbox/publish-log.jsonl`.

`jsonl` means JSON Lines: one JSON object per line. This keeps history append-only and easy to inspect with shell tools.

## Command Behavior

- Output is JSON for scripting and machine parsing.
- Exit codes:
  - `0`: success
  - `2`: validation error
  - `3`: missing auth/config
  - `4`: local IO error
  - `5`: HTTP/API error
