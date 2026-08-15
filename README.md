# Outbox

Local-first CLI for publishing content from files.

Current status: LinkedIn text publishing MVP in progress.

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

3. Follow LinkedIn app setup and OAuth steps in the next sections.

4. Publish from a local text file:

```bash
cargo run -- publish linkedin --file ./your-post.md
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
LINKEDIN_CLIENT_ID=
LINKEDIN_CLIENT_SECRET=
LINKEDIN_REDIRECT_URI=http://localhost:8788/callback
LINKEDIN_SCOPES='w_member_social openid profile'
LINKEDIN_ACCESS_TOKEN=
LINKEDIN_REFRESH_TOKEN=
LINKEDIN_ACCESS_TOKEN_EXPIRES_IN=
LINKEDIN_REFRESH_TOKEN_EXPIRES_IN=
LINKEDIN_AUTHOR_URN=urn:li:person:
LINKEDIN_API_VERSION=202607
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

2. Start OAuth login:

```bash
cargo run -- auth linkedin login
```

Optional: auto-open default browser (also prints URL):

```bash
cargo run -- auth linkedin login --open-browser
```

Expected JSON:

```json
{
  "auth_url": "https://www.linkedin.com/oauth/v2/authorization?...",
  "browser_opened": true,
  "mode": "auth_login",
  "next_command": "outbox auth linkedin exchange --code <code-from-redirect-url> --state <state-from-login>",
  "note": "Open auth_url in a browser, approve access, then copy the code query parameter from redirect URL.",
  "ok": true,
  "platform": "linkedin",
  "state": "<state>"
}
```

3. After approval, LinkedIn redirects to your `redirect_uri`.
   Copy the `code` query parameter from that URL and run:

```bash
cargo run -- auth linkedin exchange --code <copied-code> --state <state-from-login>
```

This stores token values in your local `.env`.

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

4. Resolve and save author URN automatically:

```bash
cargo run -- auth linkedin whoami
```

Expected JSON:

```json
{
  "author_urn": "urn:li:person:<id>",
  "author_urn_saved_to_env": true,
  "mode": "auth_whoami",
  "name": "<member name>",
  "next": {
    "command": "outbox publish linkedin --file <path>",
    "message": "Author URN is ready. You can publish now."
  },
  "ok": true,
  "platform": "linkedin"
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
  "published_at": "2026-08-15T12:34:56Z"
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

Direct post URL format from returned `post_id`:

```text
https://www.linkedin.com/feed/update/<post_id>/
```

Example:

```text
https://www.linkedin.com/feed/update/urn:li:share:7494283356650733568/
```

## Command Behavior

- Output is JSON for scripting and machine parsing.
- Exit codes:
  - `0`: success
  - `2`: validation error
  - `3`: missing auth/config
  - `4`: local IO error
  - `5`: HTTP/API error
