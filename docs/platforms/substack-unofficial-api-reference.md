# Pinned Substack Notes API Reference

Publo's direct Rust integration is based on the following reviewed community SDK:

- Repository: `https://github.com/cucoleadan/unofficial-substack-sdk`
- Version: `0.3.12`
- Commit: `fc5a4cd4fc62afdb851b5fe8cc72c3b9f04ee3b3`
- Reviewed: `2026-08-31`
- License: MIT

This repository is a development reference, not a Publo runtime dependency or Git
submodule. Publo sends authenticated requests directly to Substack.

Verification status as of August 31, 2026:

- Authentication/profile lookup: live-verified.
- Text-only Note publish: live-verified and confirmed on the account feed.
- Image Note publish: live-verified and confirmed on the account feed.

## Ported contracts

| SDK method | Observed request used by Publo |
|---|---|
| `getAuthenticatedProfile()` | `GET https://substack.com/api/v1/handle/options`, then `GET /api/v1/user/{handle}/public_profile` |
| `uploadImage(dataUrl)` | `POST https://substack.com/api/v1/image` with `{ "image": "data:<mime>;base64,..." }` |
| `createImageAttachment(image)` | `POST https://substack.com/api/v1/comment/attachment/` with `{ "url": image.url, "type": "image" }` |
| `createAttachment({ type: "link", url })` | `POST https://substack.com/api/v1/comment/attachment/` with `{ "url": url, "type": "link" }` |
| `publishNote(request)` | `POST https://substack.com/api/v1/comment/feed/` |

All requests send `Accept: application/json` and the cookie
`substack.sid=<SUBSTACK_SESSION_TOKEN>`. Requests also send an observed Google Chrome on macOS
`User-Agent`, `Origin: https://substack.com`, and `Referer: https://substack.com/notes` headers
because Substack can reject default non-browser clients with HTTP 403. JSON writes send
`Content-Type: application/json`. Redirects are disabled.

The trailing write-path slashes and browser-compatible headers are compatibility
hardening based on current independent request captures documented by
`AnthonyDavidAdams/substack-api-reference` and `adelaidasofia/substack-mcp`. They
are covered by Publo's local contract tests and should be reviewed with the pinned
SDK whenever Substack's behavior changes.

The Note payload is:

```json
{
  "bodyJson": {
    "type": "doc",
    "attrs": { "schemaVersion": "v1", "title": null },
    "content": []
  },
  "tabId": "for-you",
  "surface": "feed",
  "replyMinimumRole": "everyone",
  "attachmentIds": []
}
```

The pinned SDK limits Note bodies to 5,000 UTF-16 code units and creates one
ProseMirror paragraph per input line. Publo reproduces those rules.

## Reviewing upstream changes

1. Clone or fetch the SDK outside the Publo repository.
2. Review changes from the pinned commit to the proposed new commit.
3. Focus on `src/core/client.ts`, `src/core/note-body.ts`,
   `src/resources/notes/index.ts`, `src/resources/profiles/index.ts`, and their tests.
4. Compare cookie names, hosts, redirects, paths, headers, payload fields, response
   IDs, and image metadata.
5. Update Publo and its mocked contract tests when required.
6. Run a supervised `whoami`, debug preview, text Note, and image Note.
7. Update the version, commit, and review date above only after those checks pass.

Never copy SDK changes blindly. The pinned code executes with a full account session,
so every update requires a source and behavior review.
