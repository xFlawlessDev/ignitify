<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="infra/traefik/fallback/ignitify-mark.svg">
    <img src="frontend/src/assets/logo/logo-black.svg" width="104" alt="Ignitify mark">
  </picture>
</p>

<h1 align="center">Ignitify</h1>

<p align="center">A fast, lightweight Rust control plane for self-hosted deployments and operations.</p>

<p align="center">
  <a href="https://github.com/xFlawlessDev/ignitify/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://github.com/xFlawlessDev/ignitify/actions/workflows/ci.yml/badge.svg?branch=main" alt="Continuous integration status"></a>
  <a href="https://github.com/xFlawlessDev/ignitify/releases"><img src="https://img.shields.io/github/v/release/xFlawlessDev/ignitify?display_name=tag&sort=semver" alt="Latest release"></a>
  <a href="https://github.com/xFlawlessDev/ignitify/releases"><img src="https://img.shields.io/github/downloads/xFlawlessDev/ignitify/total?label=total%20installs" alt="Total release downloads"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-2ea44f" alt="Dual MIT or Apache-2.0 license"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/backend-Rust%202024-dea584" alt="Rust 2024 backend"></a>
  <a href="https://vuejs.org/"><img src="https://img.shields.io/badge/frontend-Vue%203-42b883" alt="Vue 3 frontend"></a>
</p>

<p align="center">
  <a href="#production-installation">Install</a>
  &middot;
  <a href="#capabilities">Capabilities</a>
  &middot;
  <a href="#security-and-operations">Security</a>
  &middot;
  <a href="#development">Develop</a>
  &middot;
  <a href="CONTRIBUTING.md">Contribute</a>
</p>

Ignitify brings projects, deployment execution, ingress, infrastructure
connections, runtime inspection, and encrypted backup operations into one web
application. It is designed for operators who need control over the machines,
credentials, and services they run.

Its Rust backend keeps the control plane lightweight and responsive while the
worker reconciles deployments and infrastructure operations on the host.

Ignitify is a self-hosted alternative to platforms such as Vercel, Railway,
Coolify, and Dokploy for teams that want a deployment experience without
giving up control of their servers, network, domains, and encrypted runtime
data.

When configured, Ignitify performs real Docker, Docker Compose, SSH, Git, DNS,
HTTP-monitoring, and S3-compatible storage operations. Install it only on
hosts and infrastructure you administer.

## Install Ignitify

```sh
curl -fsSL https://raw.githubusercontent.com/xFlawlessDev/ignitify/main/install.sh | sh
```

For supported Linux `amd64` hosts. The installer downloads and verifies the
latest release; see [Production installation](#production-installation) for
requirements and version selection.

## Contents

- [Capabilities](#capabilities)
- [At a glance](#at-a-glance)
- [Footprint benchmark](#footprint-benchmark)
- [Quick start](#quick-start)
- [AI assistant](#ai-assistant)
- [Notification channels](#notification-channels)
- [Compose template catalog](#compose-template-catalog)
- [Architecture](#architecture)
- [Production installation](#production-installation)
- [First operator setup](#first-operator-setup)
- [Domain routing](#domain-routing)
- [Security and operations](#security-and-operations)
- [API documentation](#api-documentation)
- [Development](#development)
- [Release packaging](#release-packaging)
- [Release guide](docs/releasing.md)
- [Repository layout](#repository-layout)
- [Contributing](#contributing)
- [License](#license)

## At A Glance

| Area | What Ignitify provides |
| --- | --- |
| Deploy | Image, reviewed Compose, and Git-backed services with queued deploys, rollback, cancellation, events, and live logs. |
| Connect | Local Docker, restricted Compose, SSH remote servers, Git providers, remote builders, Traefik, and DNS verification. |
| Operate | Container inspection and actions, controlled terminals, host metrics, uptime monitoring, remote-agent heartbeats, and delivery notifications. |
| Protect | Encrypted secrets, role-gated controls, step-up authentication, audit context, a loopback-only backend, and offline-compatible backup and restore. |

The HTTP API records intent and serves the embedded Vue application. Background
workers and dedicated adapters carry out external effects, keeping Docker,
Compose, SSH, Git, DNS, and ingress work out of request handlers. Read the
[architecture overview](#architecture) for the runtime boundary.

## Footprint Benchmark

The latest controlled idle snapshot used separate Ubuntu 24.04 VPS hosts with
one vCPU and 1,968 MiB of memory. It collected 12 five-second samples after
removing the synthetic workload. Ignitify was installed from the
checksum-verified `v0.2.1` release; Coolify was `v4.3.5` from its official
installer. The retained Dokploy column is from the validated snapshot before
that VPS was repurposed for Coolify. [Coolify documents](https://coolify.io/docs/get-started/installation)
two CPU cores and 2 GiB as its minimum recommended host, so this one-vCPU result
is a constrained-footprint observation, not a production-sizing recommendation.

| Metric | Ignitify v0.2.1 | Dokploy v0.30.0 (prior snapshot) | Coolify v4.3.5 |
| --- | ---: | ---: | ---: |
| Average host CPU | 0.50% | 2.84% | 20.42% |
| Average host memory used (`MemTotal - MemAvailable`) | 515.0 MiB | 1,404.6 MiB | 895.1 MiB |
| Average available memory | 1,453.0 MiB | 563.4 MiB | 1,072.9 MiB |
| Running platform containers | 3 | 3 | 6 |
| Platform-container memory snapshot | 170.0 MiB | 1,033.8 MiB | 508.4 MiB |
| Docker image storage after cleanup | 350.8 MB | 3.911 GB | 2.071 GB |
| Local health endpoint | HTTP 200 | HTTP 200 | HTTP 200 |

This is evidence from one controlled host pair, not a feature-equivalence,
throughput, public-latency, or reliability claim. The same validation also
exercised a digest-pinned Nginx Compose workload through Coolify's API: it
became HTTP-ready in 15.061 seconds, then product cleanup removed the container
and resource in 7.765 seconds. The Dokploy and Coolify snapshots are separate
runs on the same normalised host configuration; see the [full benchmark methodology and evidence](docs/operations/benchmark-baseline.md).

## Quick Start

| Goal | Start here |
| --- | --- |
| Install on a supported Linux host | Run the verified release installer in [Production installation](#production-installation). |
| Create the first operator and deploy a service | Follow [First operator setup](#first-operator-setup). |
| Put Ignitify behind a public domain | Review [Security and operations](#security-and-operations) before enabling remote mode or managed routes. |
| Build, test, or contribute | Read [Development](#development), then [Contributing](CONTRIBUTING.md). |

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
- Operator-managed notification channels for deployment and backup events via
  Telegram, Discord, SMTP, Resend, or custom HTTPS webhooks.
- An operator-configured OpenAI-compatible AI assistant with a global chat
  panel, chat continuation, response copy/regeneration, and context-aware
  questions from deployment, container, and terminal logs.
- Offline SQLite and runtime-secret backup/restore with optional
  S3-compatible upload, scheduled backup runs, and run history.

## AI Assistant

Platform operators configure the assistant in **AI assistant**. Ignitify calls
the configured provider's OpenAI Chat Completions endpoint
(`POST /v1/chat/completions`) with the selected model. Set a provider base URL
that is either an origin or ends in `/v1`; public providers must use HTTPS,
while loopback endpoints can use HTTP for local compatible providers.

The API key is optional for local providers, encrypted at rest when provided,
and never returned to the browser. AI configuration is operator-only. Every
authenticated user can use the floating assistant after it is enabled.

The conversation continues while the browser session remains open: each new
turn includes the current in-memory history. It is not persisted, is cleared
by the conversation clear action or page refresh, and accepts at most 32
messages per request. Users can copy messages and regenerate the latest
assistant response. Chat requests are limited to 20 requests per minute per
authenticated user.

Use **Ask AI** from deployment logs, container logs, or terminal output to
open the assistant with that output attached as context. Only the message and
log text explicitly submitted are sent to the configured provider. The
assistant is a diagnostic aid: it cannot run commands or change
infrastructure, and log text is treated as untrusted data rather than
instructions.

## Notification Channels

Platform operators can configure notification channels in **Notifications**.
Each channel can be enabled independently and subscribed to one or more
deployment events (`queued`, `preparing`, `running`, `healthy`, `failed`,
`stopping`, `stopped`, and `superseded`) and backup outcomes (succeeded or
failed).

Ignitify encrypts channel credentials at rest and returns only non-sensitive
configuration summaries through the API. Delivery runs in the background,
records each event/channel delivery to prevent duplicates, and applies a
15-second timeout. Custom webhooks must use a public HTTPS endpoint; private
and loopback hosts are rejected to preserve SSRF protections.

## Compose Template Catalog

Deploy-ready Compose blueprints are maintained separately in the
[Ignitify Templates repository](https://github.com/xFlawlessDev/ignitify-templates).
Each `blueprints/<template-id>/` directory contains a reviewed
`docker-compose.yml`, `meta.json`, and `template.toml`, with optional logos and
operator instructions. The catalog also preserves upstream links, attribution,
and license obligations for imported projects.

The Ignitify template picker reads a catalog API and fetches the matching
blueprint files from that catalog. Configure `VITE_TEMPLATES_URL` for the
catalog API used by a frontend build; the API should be backed by the
`xFlawlessDev/ignitify-templates` repository. Template updates are released
and validated in that repository independently of the Ignitify application
binary.

## Architecture

The Rust workspace is the control plane backend and Vue 3 provides the web
application. The backend embeds the production frontend bundle, so a deployed
release is served by the Ignitify process rather than a separate frontend
server.

```text
Browser
  |
  +-- Ignitify-managed Traefik HTTPS route (or an external TLS reverse proxy)
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
curl -fsSL https://raw.githubusercontent.com/xFlawlessDev/ignitify/main/install.sh | sh
```

The bootstrapper is served from GitHub raw content. It supports Linux `amd64`,
downloads the bundle and `SHA256SUMS` from the repository's
GitHub Release at `/releases/latest/download/`, verifies the archive, and
requests `sudo` when administrator access is required.

On Ubuntu, Debian, and Fedora, the release installer provisions the runtime
dependencies required by the supported local features:

- Docker Engine, Docker Compose, and Docker Buildx;
- Git and OpenSSH client;
- the matching Railpack binary included in the release archive;
- curated Traefik operator assets and writable runtime fallback-page storage;
- a dedicated `ignitify` system account, persistent data directory, root-only
  runtime configuration, and `ignitify.service`.

It starts the service by default and enables local Docker-host source builds
and the bundled Traefik operator. It does not remove an existing conflicting
Docker installation or modify its workloads. Resolve that manually before
rerunning the installer.

Select a published version instead of `latest` with:

```sh
curl -fsSL https://raw.githubusercontent.com/xFlawlessDev/ignitify/main/install.sh | sh -s -- --release vX.Y.Z
```

Useful service commands:

```sh
sudo systemctl status ignitify
sudo systemctl restart ignitify
sudo journalctl -u ignitify -f
```

The installer deliberately binds the application to `127.0.0.1:5656`. Do not
expose that port directly to the Internet.

## First Operator And VPS Setup

1. Retrieve the generated bootstrap secret from the host:

   ```sh
   sudo awk -F= '/^IGNITIFY_BOOTSTRAP_SECRET=/{print $2}' /etc/ignitify/ignitify.env
   ```

2. For a remote host before its admin domain is configured, create an SSH
   tunnel from your local computer and keep it open:

   ```sh
   ssh -L 5656:127.0.0.1:5656 <ssh-user>@<server-address>
   ```

   Then open `http://127.0.0.1:5656` in your local browser. The application
   remains loopback-only on the remote host.
3. Open Ignitify through the local host or active SSH tunnel and use that secret
   to create the first operator account.
4. Point your domain at the VPS. For the simplest setup, create a wildcard DNS
   record such as `*.yourdomain.com` with type `A` and the VPS public IPv4
   address as its target. This lets Ignitify route every service hostname under
   that suffix through its managed reverse proxy. Add an `AAAA` record when
   applicable. If the control-plane hostname is outside that suffix, create a
   separate record for it, for example `console.example.com`.
   Allow public TCP ports `80` and `443` through both the VPS and provider
   firewall.
5. In **Infrastructure > Ingress & TLS**, set that control-plane hostname, the
   application domain suffix, an ACME contact email, and automatic certificates
   (or select a custom certificate). Ignitify generates the Traefik HTTPS route
   to its loopback-only backend. The control-plane hostname may be within the
   managed application suffix, but is reserved from service routes.
6. Add Git providers, remote servers, remote builders, or S3-compatible backup
   destinations only when their credentials and network access are ready.
7. Create a project and service, choose an image, Compose, or Git source, then
   deploy. Review the deployment events and logs after every change.

## Managed Admin Domain

The bundled Traefik operator is the normal way to expose the Ignitify console
on a public VPS. After the control-plane hostname is saved, Ignitify manages the
HTTPS route, enables that HTTPS origin for browser requests, and accepts the
forwarded headers from its own proxy. No manual reverse-proxy configuration or
environment-variable change is needed. Keep port `5656` private.

The managed control-plane domain requires `IGNITIFY_SECURE_COOKIES=true`, which
is already the installation default. Use `IGNITIFY_REMOTE_MODE`,
`IGNITIFY_TRUST_PROXY_HEADERS`, and an HTTPS `IGNITIFY_TRUSTED_ORIGINS` value
only when placing a different external reverse proxy or tunnel in front of the
application. Domain names, TLS certificates, ACME contact details, DNS
credentials, Git tokens, SSH keys, and S3 credentials are not invented by the
installer and must be provided by the operator.

## Domain Routing

Ignitify uses its bundled Traefik instance as the reverse proxy for the console
and deployed services. Configure an application suffix such as
`yourdomain.com`, then point `*.yourdomain.com` at the public IP address of the
Ignitify host with an `A` record. Service hostnames such as
`api.yourdomain.com` and `web.yourdomain.com` can then be routed without adding
one DNS record per service.

The wildcard record does not cover the apex `yourdomain.com`; create a separate
apex record only when you intend to use it. DNS providers that offer a proxy
mode may proxy the wildcard record only when their configuration passes HTTPS
traffic through to Ignitify on ports `80` and `443`; use DNS-only mode while
validating direct ACME certificates. Ignitify still owns the application routes
and TLS configuration, and port `5656` must remain private.

## Security And Operations

- The `ignitify` service account is placed in the local `docker` group so that
  it can deploy containers. Docker socket access is effectively host-privileged;
  restrict host and repository access accordingly.
- The backend is loopback-only by design. Keep remote mode behind HTTPS and
  permit only explicit trusted origins.
- Keep `/etc/ignitify/ignitify.env`, `/var/lib/ignitify`, backups, generated
  certificates, provider credentials, private keys, and database copies out of
  version control and out of untrusted storage.
- Treat the configured AI provider as a data recipient. Submit only log output
  and questions appropriate for that provider, and do not place API keys or
  other credentials in chat messages or attached log context.
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

## API Documentation

The running backend serves an OpenAPI 3.1 document and Swagger UI without a
separate documentation service:

- Swagger UI: `https://<ignitify-host>/swagger-ui/`
- OpenAPI JSON: `https://<ignitify-host>/api-docs/openapi.json`

The published document covers every registered API route, grouped by domain.
Health and S3 backup operations include typed request and response schemas;
the remaining routes expose their HTTP method, path parameters, authentication,
and mutation requirements while their DTO schemas are annotated incrementally.
The UI is publicly readable, but every protected operation still enforces its
normal backend authentication and authorization checks. Use **Authorize** in
Swagger UI and enter an operator JWT access token without the `Bearer ` prefix.
For every browser-initiated state-changing endpoint, set the documented
`X-Ignitify-Request` header parameter to `1`; trusted-origin validation still
applies. The remote-agent heartbeat instead uses its provisioned agent bearer
token. Never enter bootstrap secrets, refresh tokens, or long-lived provider
credentials into browser tooling.

## Development

### Prerequisites

- Rust toolchain `1.95` or newer;
- Node.js `22.12` or newer with pnpm `11.18.0`;
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

The workspace version in root `Cargo.toml` is the source metadata for all Rust
crates. `scripts/version.sh` derives an effective version from an exact
`vX.Y.Z` Git tag, or creates a deterministic development snapshot from the
current commit. Synchronize a deliberately chosen next source version before
committing and tagging it:

```sh
bash scripts/version.sh --set 0.2.0
git add Cargo.toml Cargo.lock frontend/package.json crates/*/Cargo.toml
git commit -m "chore: prepare v0.2.0"
git tag v0.2.0
```

Build the embedded frontend, validate the workspace, and package a native
Linux release with the matching Railpack binary:

```sh
bash scripts/build-release.sh --require-tag --railpack /path/to/railpack
```

This creates:

```text
dist/vX.Y.Z/ignitify-linux-amd64.tar.gz
dist/vX.Y.Z/SHA256SUMS
dist/vX.Y.Z/release-linux-amd64.json
```

Pushing an exact `vX.Y.Z` tag starts the GitHub Actions release workflow. It
builds and validates the native `amd64` archive, verifies the pinned Railpack
binary, writes `SHA256SUMS`, and publishes the GitHub Release. The separate CI
workflow runs the frontend and Rust quality gates for pull requests and pushes
to `main`.

See the [release guide](docs/releasing.md) for the one-time GitHub setup,
version and tag procedure, verification steps, manual fallback, and
troubleshooting.

Before enabling releases, configure the repository's GitHub Actions workflow
permissions to allow `contents: write`. The publish job targets the
`production` environment; configure required reviewers there when release
approval is needed. The default installer resolves assets through
`https://github.com/xFlawlessDev/ignitify/releases/latest/download/`; a
versioned installation uses
`https://github.com/xFlawlessDev/ignitify/releases/download/vX.Y.Z/`.
ARM64 delivery is intentionally disabled until it has been validated on native
hardware. The installer reports this limitation explicitly on ARM64 hosts.

For an emergency manual build, run `scripts/build-release.sh` on each native
Linux architecture. Use `--dry-run` to inspect its release plan, or
`--skip-check` only when an equivalent CI quality gate already ran.

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
  ignitify-notifications/    Notification event delivery adapters
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
