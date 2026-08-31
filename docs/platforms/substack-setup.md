# Substack Notes Setup

Publo supports Substack Notes only. It does not create or publish Substack articles.

Current verification status:

- Text-only Notes: live-verified on August 31, 2026.
- Image Notes: live-verified on August 31, 2026.
- Scheduled/worker publishing: text Note live-verified on August 31, 2026; dry-run and
  scheduled image-Note verification remain.

Substack does not provide Publo with an OAuth publishing flow. Publo therefore uses
the authenticated browser session used by Substack's observed web API. This is an
unofficial integration and can break when Substack changes that API.

## Configure the session

1. Sign in to `https://substack.com` in your browser.
2. Open browser developer tools.
3. Open Application/Storage, then Cookies, then `https://substack.com`.
4. Find `substack.sid` and copy only its value.
5. Open the active Publo workspace `.env` file. Run `publo paths` if its location is unknown.
6. Add:

```dotenv
SUBSTACK_SESSION_TOKEN='s%3A...'
SUBSTACK_PUBLICATION_URL='https://yourname.substack.com'
```

Do not include `substack.sid=` and do not paste the complete Cookie header. The
session grants account access and must be treated like a password. Never pass it as
a command argument, commit it, paste it into logs, or include it in bug reports.

There is no refresh command. If the session expires, copy the new `substack.sid`
value from the browser and replace `SUBSTACK_SESSION_TOKEN`.

## Verify authentication

```bash
publo auth substack guide
publo auth substack session-status
publo auth substack whoami
```

`session-status` validates local configuration and always redacts the session value.
`whoami` makes authenticated requests directly to `https://substack.com`.

## Preview and publish

```bash
publo publish substack --file ./note.md --debug
publo publish substack --file ./note.md --pass <publish-pass>
publo publish substack --file ./note-with-images.md --pass <publish-pass>
```

The shared options are:

- `--timeout-seconds`
- `--allow-duplicate`
- `--debug`
- `--add-signature`
- `--no-signature`

Publo uses the same file contract as LinkedIn and X: publish text comes after the
last `---`, and Obsidian `![[...]]` embeds are resolved as images. Supported image
extensions are `.png`, `.jpg`, and `.jpeg`.

The final create-Note request is never automatically retried. If the connection is
lost after sending it, Publo reports an unknown outcome; inspect your Substack
profile before publishing again.

Substack may reject a correct cookie-based write from a default non-browser HTTP
client. Publo sends the currently observed Chrome-on-macOS user agent, Substack
origin/referrer headers, and canonical write paths for compatibility. A definitive
HTTP 403 is reported as rejected; a transport failure after sending remains an
unknown outcome.

## Scheduled and worker publishing

Substack is accepted by job scheduling and `publo worker run` as
`publish_mode = "note"`. It reuses the existing job lifecycle, atomic claim, immutable
publish attempts, and uncertain-outcome protections. Migration `0002` removes the closed
publish-mode allowlist while preserving existing data, allowing `note` and future modes
without additional schema migrations. The initial migration remains unchanged.

Before relying on scheduled Substack publishing, complete the supervised sequence:

```bash
publo worker run --dry-run --once
publo worker run --live --once --pass <publish-pass>
```

The live command processes at most one oldest-due job. Confirm the resulting Note on
Substack and inspect its job and attempt details in Publo.
