# Publo

Your personal publishing pipeline.  
Your content stays local. Publo takes it from there.

Vision and direction: [vision.md](vision.md)
Quick setup guide: [GETTING_STARTED.md](GETTING_STARTED.md)
Platform setup guides:

- [docs/platforms/linkedin-setup.md](docs/platforms/linkedin-setup.md)
- [docs/platforms/x-setup.md](docs/platforms/x-setup.md)

## Current status

- LinkedIn: text + image + multi-image publishing
- X: text + image publishing (up to 4 images)
- Guided OAuth login for both platforms
- Manual OAuth exchange fallback for both platforms
- Token status/refresh commands for both platforms
- Local-first parsing for Obsidian notes (`---` split + `![[...]]`)
- Duplicate guard + JSONL publish log

## Feature matrix

| Capability | LinkedIn | X |
|---|---|---|
| Text post | ✅ | ✅ |
| Single image | ✅ | ✅ |
| Multi-image | ✅ (2-20) | ✅ (1-4) |
| Guided OAuth callback login | ✅ | ✅ |
| Manual OAuth exchange | ✅ | ✅ |
| Token status | ✅ | ✅ |
| Token refresh | ✅ | ✅ |
| Auto refresh during publish on 401 | ✅ | ✅ |
| Debug no-send mode (`--debug`) | ✅ | ✅ |

## Prerequisites

- Rust toolchain (`cargo`)
- LinkedIn and/or X developer app

If you are running directly from source without installing the binary, prepend commands with `cargo run --`  
(example: `cargo run -- auth x login`).

## Quick start

1. Copy env template:

```bash
cp .env.example .env
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
```

4. Validate without posting:

```bash
publo publish linkedin --file ./post.md --debug
publo publish x --file ./post.md --debug
```

5. Publish:

```bash
publo publish linkedin --file ./post.md
publo publish x --file ./post.md
```

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

[media]
lookup_paths = ["/absolute/path/to/media-assets"]
```

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
publo publish linkedin --file ./post.md
```

Options:

- `--allow-duplicate`
- `--debug`
- `--add-signature`
- `--no-signature`

Image limits:

- single image: supported
- multi-image: supported, max 20
- allowed extensions: `.png`, `.jpg`, `.jpeg`

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
publo publish x --file ./post.md
```

Options:

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
