# Getting Started

This is the fastest path for a clean local setup.

If you run from source, prepend commands with `cargo run --`.

## 1) First-time setup (new user, first workspace)

Create your first workspace:

```bash
publo init --display-name "My Personal Workspace"
```

Verify paths:

```bash
publo paths
```

Edit workspace config (use your own editor):

```bash
$EDITOR ~/.publo.so/workspaces/my-personal-workspace/config.toml
```

Set your content/media roots:

```toml
[catalog]
roots = ["/absolute/path/to/content"]

[media]
lookup_paths = ["/absolute/path/to/assets"]
```

Fill platform keys/tokens (use your own editor):

```bash
$EDITOR ~/.publo.so/workspaces/my-personal-workspace/.env
```

Before filling `.env`, create your platform developer apps and collect credentials/tokens:

- LinkedIn: [docs/platforms/linkedin-setup.md](docs/platforms/linkedin-setup.md)
- X: [docs/platforms/x-setup.md](docs/platforms/x-setup.md)

Run platform auth:

```bash
publo auth linkedin login
publo auth x login
```

Run safe prechecks (no post sent):

```bash
publo publish linkedin --file /absolute/path/to/post.md --debug
publo publish x --file /absolute/path/to/post.md --debug
```

Start local API/backend:

```bash
publo serve
```

## 2) Add a new business/workspace

Create another workspace:

```bash
publo init --workspace-id acme-client --display-name "Acme Client"
```

Switch default workspace:

```bash
publo workspace switch --workspace-id acme-client
```

Check active paths:

```bash
publo paths
```

Then configure the new workspace files:

- `~/.publo.so/workspaces/acme-client/config.toml`
- `~/.publo.so/workspaces/acme-client/.env`

## 3) One-off command in a non-default workspace

Use env override without switching global default:

```bash
PUBLO_WORKSPACE=acme-client publo paths
PUBLO_WORKSPACE=acme-client publo auth linkedin login
PUBLO_WORKSPACE=acme-client publo serve
```
