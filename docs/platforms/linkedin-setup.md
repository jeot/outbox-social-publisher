# LinkedIn App Setup

Use this once per workspace to get LinkedIn credentials and runtime tokens.

## 1) Create app

Go to LinkedIn Developer Portal and create/select your app.

In Products, enable:

- Share on LinkedIn
- Sign In with LinkedIn using OpenID Connect

## 2) OAuth settings

Add redirect URL (must match `.env` exactly):

- `http://localhost:8788/callback`

## 3) Fill workspace `.env`

Open your workspace env file:

```bash
$EDITOR ~/.publo.so/workspaces/<workspace-id>/.env
```

Set:

```dotenv
LINKEDIN_CLIENT_ID=
LINKEDIN_CLIENT_SECRET=
LINKEDIN_REDIRECT_URI=http://localhost:8788/callback
LINKEDIN_SCOPES='w_member_social openid profile'
LINKEDIN_API_VERSION=202607
```

Runtime values are written by auth flow:

```dotenv
LINKEDIN_ACCESS_TOKEN=
LINKEDIN_REFRESH_TOKEN=
LINKEDIN_ACCESS_TOKEN_EXPIRES_IN=
LINKEDIN_REFRESH_TOKEN_EXPIRES_IN=
LINKEDIN_AUTHOR_URN=
```

## 4) Run auth

```bash
publo auth linkedin login
```

Manual fallback:

```bash
publo auth linkedin exchange --code <code> --state <state>
publo auth linkedin whoami
```

Utilities:

```bash
publo auth linkedin token-status
publo auth linkedin token-refresh
```
