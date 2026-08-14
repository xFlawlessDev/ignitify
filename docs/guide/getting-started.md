# Getting started locally

Ignitify consists of a control plane (`ignitify/`) and an administrator dashboard (`ignitify/frontend/`). Run the components required for your work.

## Prerequisites

- Rust stable with edition 2024 and Cargo.
- Node.js 22 or newer and Corepack/pnpm.
- Docker Engine, Docker Compose v2, and Docker Buildx for running deployments.
- Git for source builds. Railpack is required for the `railpack` builder.

## Set up the control plane

From the `ignitify` repository root:

```powershell
Copy-Item .env.example .env
cargo check --workspace
cargo test --workspace
```

`IGNITIFY_JWT_SECRET` and the age identity do not need to be set for development: the backend creates and stores the first runtime secrets in `IGNITIFY_DATA_DIR` (default `data/`). For a persistent environment, set explicit values and protect that data directory.

Run the API when you need a local runtime:

```bash
cargo run -p ignitify-core
```

The API listens at `http://127.0.0.1:5656`; the unauthenticated `GET /health` endpoint is available for a basic process check.

## Set up the administrator dashboard

From `ignitify/frontend`:

```bash
corepack enable
vp install
vp run check
vp run test
vp dev
```

The dashboard is available at `http://127.0.0.1:6565` and proxies `/api` to backend port `5656`, including the terminal WebSocket. When the database has no users, the login screen asks you to bootstrap the first administrator.

## First bootstrap

Bootstrap can only happen once. From the dashboard, create the administrator username and password. API clients need a trusted origin and the `X-Ignitify-Request: 1` header for state-changing requests:

```powershell
$body = @{ username = "admin"; password = "use-a-strong-password" } | ConvertTo-Json
Invoke-RestMethod -Method Post `
  -Uri "http://127.0.0.1:5656/api/v1/auth/bootstrap" `
  -Headers @{ Origin = "http://127.0.0.1:6565"; "X-Ignitify-Request" = "1" } `
  -ContentType "application/json" `
  -Body $body
```

Keep the access token returned by the response in client memory only. The server manages the refresh token as an HttpOnly cookie.

## Recommended checks

| Area                       | Command                                                 |
| -------------------------- | ------------------------------------------------------- |
| Rust format                | `cargo fmt --all -- --check`                            |
| Rust lint                  | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust tests                 | `cargo test --workspace`                                |
| Dashboard format/lint/type | `vp run check` from `ignitify/frontend`                 |
| Dashboard tests            | `vp run test` from `ignitify/frontend`                  |

Continue to [configuration](/operations/configuration) before enabling Docker, ingress, or Git repository builds.
