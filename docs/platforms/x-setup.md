# X App Setup

Use this once per workspace to get X credentials and runtime tokens.

## 1) Create app

Go to `https://console.x.com/` and create/select your app.

In **OAuth 2.0 user authentication settings**:

- App type: `Native App` (public client)
- App permissions: `Read and write`
- Callback URL: `http://127.0.0.1:8789/callback`
- Scopes include: `tweet.read tweet.write users.read media.write offline.access`

## 2) Billing

X API is pay-per-usage. Add credit in:

- `console.x.com` → Billing → Credits

Without credits you may get:

- HTTP `402`
- `credits depleted`

## 3) Fill workspace `.env`

Open your workspace env file:

```bash
$EDITOR ~/.publo.so/workspaces/<workspace-id>/.env
```

Set:

```dotenv
X_CLIENT_ID=
X_CLIENT_SECRET=
X_REDIRECT_URI=http://127.0.0.1:8789/callback
X_SCOPES='tweet.read tweet.write users.read media.write offline.access'
```

Runtime values are written by auth flow:

```dotenv
X_ACCESS_TOKEN=
X_REFRESH_TOKEN=
X_ACCESS_TOKEN_EXPIRES_IN=
X_TOKEN_TYPE=
```

## 4) Run auth

```bash
publo auth x login
```

Manual fallback:

```bash
publo auth x exchange --code <code> --state <state>
```

Utilities:

```bash
publo auth x token-status
publo auth x token-refresh
```
