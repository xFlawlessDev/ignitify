# Ignitify

Ignitify is a self-hosted deployment and operations control plane. It lets an
operator manage projects, build and deploy services, route domains through
Traefik, connect Git providers and remote servers, inspect runtime state, and
retain encrypted backups from a single web application.

Ignitify runs real Docker, Docker Compose, SSH, Git, DNS, HTTP-monitoring, and
S3-compatible storage operations when configured. Install it only on hosts and
infrastructure you administer.

## Contents

- [Capabilities](#capabilities)
- [Architecture](#architecture)
- [Production installation](#production-installation)
- [First operator setup](#first-operator-setup)
- [Security and operations](#security-and-operations)
- [Development](#development)
- [Release packaging](#release-packaging)
- [Repository layout](#repository-layout)
- [Contributing](#contributing)
- [License](#license)

## Capabilities

- Password bootstrap/login, JWT access tokens, rotating refresh sessions,
  step-up authentication, audit context, and role-based operator controls.
- Projects, encrypted project and service variables, and image, Compose, or
  Git-backed service definitions.
- Queued deployments with rollback, cancellation, stopping, events, and live
  server-sent deployment logs.
- Docker, restricted Compose, and SSH remote-server runtimes, plus Git source
  builds using Dockerfiles, static sites, Railpack, or reviewed Compose input.
- Traefik ingress, managed routes and certificates, DNS verification, domain
  policy, ACME settings, and configurable fallback pages.
- GitHub, GitLab, and Gitea integrations; repository and branch discovery;
  remote BuildKit builders; runtime inspection and actions; controlled
  terminals; host metrics; uptime monitoring; and remote-agent heartbeats.
- Offline SQLite and runtime-secret backup/restore with optional
  S3-compatible upload, scheduled backup runs, and run history.

## Architecture

The Rust workspace is the control plane backend and Vue 3 provides the web
application. The backend embeds the production frontend bundle, so a deployed
release is served by the Ignitify process rather than a separate frontend
server.

```text
Browser
  |
  +-- TLS reverse proxy (required for remote access)
        |
        +-- Ignitify API and embedded Vue application (127.0.0.1:5656)
              |
              +-- SQLite state and encrypted runtime secrets
              +-- deployment worker
                    |-- Docker / restricted Compose
                    |-- SSH remote servers and remote builders
                    |-- Git source build executor
                    |-- Traefik operator stack and DNS verifier
                    +-- monitoring and S3-compatible backup upload
```

The HTTP layer creates and reads control-plane state. Workers and runtime
adapters perform external effects and retries. This separation is intentional:
do not invoke Docker, Compose, SSH, Git, DNS, or ingress operations directly
from HTTP handlers.

## Production Installation

The default Linux installation is a single command:

```sh
curl -fsSL https://ignitify.xflawless.dev/install.sh | sh
```

The bootstrapper selects the `amd64` or `arm64` archive, downloads the release
and `SHA256SUMS` from `/releases/latest/`, verifies the archive, and requests
`sudo` when administrator access is required.

On Ubuntu, Debian, and Fedora, the release installer provisions the runtime
dependencies required by the supported local features:

- Docker Engine, Docker Compose, and Docker Buildx;
- Git and OpenSSH client;
- the matching Railpack binary included in the release archive;
- curated Traefik operator assets;
- a dedicated `ignitify` system account, persistent data directory, root-only
  runtime configuration, and `ignitify.service`.

It starts the service by default and enables local Docker-host source builds
and the bundled Traefik operator. It does not remove an existing conflicting
Docker installation or modify its workloads. Resolve that manually before
rerunning the installer.

Select a published version instead of `latest` with:

```sh
curl -fsSL https://ignitify.xflawless.dev/install.sh | sh -s -- --release vX.Y.Z
```

Useful service commands:

```sh
sudo systemctl status ignitify
sudo systemctl restart ignitify
sudo journalctl -u ignitify -f
```

The installer deliberately binds the application to `127.0.0.1:5656`. Do not
expose that port directly to the Internet.

## First Operator Setup

1. Retrieve the generated bootstrap secret from the host:

   ```sh
   sudo awk -F= '/^IGNITIFY_BOOTSTRAP_SECRET=/{print $2}' /etc/ignitify/ignitify.env
   ```

2. Open Ignitify through the local host or a TLS reverse proxy and use that
   secret to create the first operator account.
3. In **Infrastructure**, set the ACME contact email, domain policy, and any
   required DNS settings before enabling public service domains.
4. Add Git providers, remote servers, remote builders, or S3-compatible backup
   destinations only when their credentials and network access are ready.
5. Create a project and service, choose an image, Compose, or Git source, then
   deploy. Review the deployment events and logs after every change.

Remote browser access needs an operator-managed TLS reverse proxy and explicit
configuration in `/etc/ignitify/ignitify.env`:

```dotenv
IGNITIFY_REMOTE_MODE=true
IGNITIFY_TRUST_PROXY_HEADERS=true
IGNITIFY_TRUSTED_ORIGINS=https://ignitify.example.com
```

`IGNITIFY_SECURE_COOKIES=true` is already the installation default. Domain
names, TLS certificates, ACME contact details, DNS credentials, Git tokens,
SSH keys, and S3 credentials are not invented by the installer and must be
provided by the operator.

## Security And Operations

- The `ignitify` service account is placed in the local `docker` group so that
  it can deploy containers. Docker socket access is effectively host-privileged;
  restrict host and repository access accordingly.
- The backend is loopback-only by design. Keep remote mode behind HTTPS and
  permit only explicit trusted origins.
- Keep `/etc/ignitify/ignitify.env`, `/var/lib/ignitify`, backups, generated
  certificates, provider credentials, private keys, and database copies out of
  version control and out of untrusted storage.
- The Traefik operator owns managed routes and certificates. Do not edit
  generated files under the configured Traefik dynamic directory by hand.
- Local source builds execute through the Docker host. Disable them with
  `--no-local-builds` during an offline/custom installation, or use a remote
  builder where that better matches the trust boundary.
- Offline restore is intentionally a stopped-service operation. See
  [the backup operations guide](infra/operations/README.md) before restoring a
  snapshot.
- Review [the Traefik guide](infra/traefik/README.md),
  [Git build guide](infra/git-build/README.md), and
  [remote builder guide](infra/remote-builder/README.md) before operating
  those features in production.

## Development

### Prerequisites

- Rust toolchain with edition 2024 support;
- Node.js with pnpm `11.18.0`;
- Docker, Git, and OpenSSH for runtime paths that exercise them.

Build the frontend before Rust builds that compile `ignitify-api`; the backend
embeds `frontend/dist` at compile time.

```sh
cd frontend
pnpm install --frozen-lockfile
pnpm run build
cd ..
cargo check --workspace
```

For a local server session, create a local `.env` from `.env.example`, set a
unique bootstrap secret, then run the backend. Do not commit `.env` or its
generated data.

```sh
cp .env.example .env
# Set IGNITIFY_BOOTSTRAP_SECRET to at least 32 random bytes in .env.
# For the HTTP Vite frontend, also set these development-only values:
# IGNITIFY_SECURE_COOKIES=false
# IGNITIFY_TRUSTED_ORIGINS=http://localhost:6565,http://127.0.0.1:6565
cargo run -p ignitify-core
```

The backend listens on `127.0.0.1:5656`. Run the frontend development server
from `frontend/` when working on the UI; it serves port `6565` and proxies API
requests to the backend.

### Quality Checks

Run Rust checks from the repository root:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Run frontend checks from `frontend/`:

```sh
pnpm run check
pnpm run build
pnpm run test
```

End-to-end scripts are present, but the repository does not currently track a
Playwright configuration or authored end-to-end suite. Do not describe E2E
coverage until that exists.

## Release Packaging

Build the embedded frontend and the Linux backend binary on the target
architecture, then package it with the matching Railpack binary:

```sh
cd frontend
pnpm install --frozen-lockfile
pnpm run build
cd ..
cargo build --locked --release -p ignitify-core
bash scripts/package-release.sh --version vX.Y.Z --railpack /path/to/railpack
```

This creates:

```text
dist/vX.Y.Z/ignitify-linux-amd64.tar.gz
dist/vX.Y.Z/SHA256SUMS
```

Publish the archive and checksum at
`https://ignitify.xflawless.dev/releases/vX.Y.Z/`. Copy the selected artifacts
to `/releases/latest/` for the default installer command. Build and publish a
separate archive for `arm64` when that architecture is supported.

Never publish `.env` files, databases, runtime secrets, generated certificates,
temporary build workspaces, or an archive without its matching checksum.

## Repository Layout

```text
crates/
  ignitify-core/             Runtime composition, CLI backup/restore, service process
  ignitify-api/              Axum routes, HTTP adapters, SPA embedding
  ignitify-auth/             Credentials, sessions, tokens, bootstrap, step-up
  ignitify-db/               SQLite, migrations, models, repositories
  ignitify-domain/           Runtime-neutral validation and domain types
  ignitify-control-plane/    Deployment worker, state, contracts, encryption
  ignitify-runtime-*/        Docker, Compose, and SSH runtime adapters
  ignitify-source-git/       Git checkout and source-build execution
  ignitify-ingress-traefik/  Traefik lifecycle and route generation
  ignitify-dns/              DNS verification
  ignitify-monitoring/       Uptime-monitoring worker
  ignitify-terminal/         Controlled PTY primitives
  ignitify-backup-s3/        S3-compatible upload implementation
frontend/                    Vue 3, Pinia, Vue Router, Tailwind application
infra/                       Operator guides and Traefik/build assets
scripts/                     Release installer and packaging scripts
install.sh                   Public POSIX release bootstrapper
```

See [AGENTS.md](AGENTS.md) for architecture, security boundaries, code
conventions, and testing requirements used by maintainers and coding agents.

## Contributing

Contributions are welcome. Ignitify can operate deployment hosts and store
sensitive infrastructure credentials, so every change must preserve clear
ownership, authorization, and safe failure behavior. Read
[CONTRIBUTING.md](CONTRIBUTING.md) for the complete workflow, security
reporting expectations, quality gates, and pull request checklist.

## License

Ignitify is available under either of the following licenses, at your option:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

By contributing, you agree that your contribution may be distributed under
those terms. This matches the workspace SPDX declaration:
`MIT OR Apache-2.0`.
