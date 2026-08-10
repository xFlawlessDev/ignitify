# Repository Guidelines

## Project Overview

Ignitify is a self-hosted deployment and operations control plane built with Rust and Vue 3. It persists state in SQLite and runs a local worker that reconciles deployments. The current product includes:

- password bootstrap/login, JWT access tokens, rotating hashed refresh sessions, step-up authentication, audit context, and role-gated operator controls;
- projects, encrypted project and service environment variables, image, Compose, and Git-backed services;
- queued deployment, rollback, cancel, stop, deployment events, and server-sent deployment logs;
- local Docker, restricted Compose, and SSH remote-server runtimes; Git source builds using Dockerfile, static, Railpack, or reviewed Compose sources;
- Traefik ingress, managed routes and certificates, DNS verification, domain policy, infrastructure settings, and ingress fallback configuration;
- GitHub/GitLab/Gitea providers, provider tests and repository/branch discovery, remote BuildKit builders, runtime container inspection/actions, controlled terminals, host metrics, uptime monitoring, and remote-agent heartbeats;
- offline SQLite/runtime-secret backup and restore, with optional S3-compatible upload, scheduled S3 runs, and backup-run history.

The product executes real Docker, Compose, SSH, DNS, HTTP-monitoring, Git, and S3 effects when configured. Treat all runtime and infrastructure code as security-sensitive. Never claim a capability is only a UI fixture unless the relevant implementation is actually absent.

## Architecture And Data Flow

- The Rust workspace uses Rust 2024 and shared dependencies from root `Cargo.toml`.
- `ignitify-core` is the binary composition root. It loads runtime secrets/configuration, dispatches `backup` and `restore` CLI operations, builds adapters and workers, binds the loopback listener, and calls `axum::serve`. It owns no HTTP routes, handlers, request/response DTOs, or `IntoResponse` mapping.
- `ignitify-api` owns Axum route registration, handlers, HTTP DTOs, request extractors, cookies/origin checks, WebSocket/SSE adapters, static frontend serving, audit context, and safe API-error mapping. A handler authenticates, authorizes, validates, calls a service/repository, records audit context where required, then maps the result.
- `ignitify-auth` owns Argon2 credentials, bootstrap and step-up flow, JWT access tokens, rotating hashed refresh-token families, session DTOs, and `AuthError`. It receives `Database` through `AuthService::new(database, config)`.
- `ignitify-db` owns SQLite connection setup, embedded migrations, persistence records, and repositories. It is the authoritative state store for users, projects, services, deployments, domains, settings, providers, remote infrastructure, monitoring, audit activity, and backup destinations.
- `ignitify-domain` owns transport-agnostic validation and domain types. It must not import SQLx, Axum, authentication, Docker, or runtime types.
- `ignitify-control-plane` owns service configuration encryption/read models, deployment submission, worker reconciliation, stream publication, and runtime/ingress/source-build traits. HTTP submits or reads state; the worker and adapters own external effects and retries.
- Runtime/infrastructure adapters implement the control-plane contracts: `ignitify-runtime-docker`, `ignitify-runtime-compose`, `ignitify-runtime-remote`, `ignitify-ingress-traefik`, `ignitify-source-git`, and `ignitify-dns`. `ignitify-monitoring` runs the uptime worker; `ignitify-terminal` owns PTY primitives; `ignitify-backup-s3` owns S3 upload signing and transport.
- Keep dependencies acyclic. The usual flow is `core -> api -> auth/control-plane/db/domain and adapters`; `control-plane -> db/domain`; adapters depend on control-plane/domain/db only where their contract requires it. Lower layers must never import `ignitify-api` or `ignitify-core`.
- Frontend flow: `src/main.ts` installs Pinia, i18n, and Router; the router initializes `useAuthStore`; API calls attach the memory-only Bearer token; refresh uses a strict HttpOnly cookie; Vite proxies `/api` in development. Production requests are served from the embedded frontend bundle by the backend.
- Backend defaults to `127.0.0.1:5656`. The Vite development server defaults to port `6565` and proxies to the backend. The backend remains loopback-only; remote access belongs behind a TLS reverse proxy with remote mode enabled.

## Current Runtime Boundaries

- Deployment execution is active. Do not place Docker, Compose, SSH, Git, DNS, or ingress effects in HTTP handlers. Extend the worker contract and the appropriate adapter instead.
- The Docker runtime manages only labelled Ignitify containers and applies resource limits. Compose input is parsed and policy-checked before execution; do not weaken the host-escape, digest-pinning, port, mount, privileged, or network restrictions without an explicit security decision and regression coverage.
- Git source builds use temporary checkouts and credentials. Never put provider secrets in service specs, deployment snapshots, generated images, logs, command arguments, or the frontend. Preserve pinned-image and timeout checks.
- Remote runtime uses SSH with strict host-key checking and temporary private-key/known-host files. Remote builders require mTLS material. Keep all private material encrypted at rest and zeroized/removed after use.
- Traefik is an operator stack. The worker owns generated routes and TLS configuration; `infra/traefik/` owns the stack files. Do not invoke Docker or Compose during agent work unless the user explicitly requests it.
- Uptime monitoring is a background worker. Validate monitor targets and preserve SSRF protections, timeouts, redirect policy, and heartbeat expiry behavior.
- `ignitify-core backup <directory>` creates an offline-compatible database and runtime-secret snapshot, then optionally uploads it to the configured S3-compatible destination. Enabled S3 destinations can schedule the same operation at a validated 1-720 hour interval and retain non-sensitive run history. `restore <directory> --confirm-offline` is deliberately offline-only. Do not add web restore, automatic remote deletion, or plaintext secret export.

## Key Directories

- `crates/ignitify-core/` - runtime configuration/secrets, dependency composition, backup/restore operations, listener, and process error.
- `crates/ignitify-api/` - Axum router, handlers, HTTP DTOs/extractors, static SPA embedding/serving, audit helpers, and API error mapping.
- `crates/ignitify-auth/` - credential, token, session, bootstrap, and step-up behavior.
- `crates/ignitify-db/` - SQLx SQLite database, numbered migrations (`0001` through `0026` are committed), models, repositories, and persistence tests.
- `crates/ignitify-domain/` - validation and runtime-neutral domain types.
- `crates/ignitify-control-plane/` - deployment worker, service control, encrypted environment handling, streams, and runtime contracts.
- `crates/ignitify-runtime-docker/`, `ignitify-runtime-compose/`, and `ignitify-runtime-remote/` - image, Compose, and SSH runtime adapters.
- `crates/ignitify-source-git/` - source checkout/build execution and provider credential use.
- `crates/ignitify-ingress-traefik/` and `ignitify-dns/` - ingress lifecycle/route generation and DNS verification.
- `crates/ignitify-monitoring/`, `ignitify-terminal/`, and `ignitify-backup-s3/` - monitoring, PTY, and backup-upload boundaries.
- `frontend/src/lib/api/` - typed API functions, authentication transport, and request behavior.
- `frontend/src/composables/` - reusable domain orchestration; `frontend/src/stores/` - Pinia state, including the setup-style `auth.ts` store.
- `frontend/src/views/` - routed application surfaces; `frontend/src/components/` - feature components and reusable UI primitives under `components/ui/`.
- `frontend/src/assets/styles/global.css` and `frontend/design.md` - runtime design tokens and the visual contract.
- `infra/` - operator documentation and Traefik, remote-builder, Git-build, and backup/restore assets. These are runtime-sensitive, not demo files.
- `thoughts/shared/` - research and design artifacts; not runtime source.

## Development Commands

Do not start backend or frontend development servers during agent work unless the user explicitly asks. In particular, do not run `cargo run -p ignitify-core`, `pnpm run dev`, Docker, or Docker Compose just to inspect behavior.

Run Rust commands from repository root:

```sh
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

`ignitify-api` embeds `frontend/dist` at Cargo build time. After frontend source changes, build the frontend before a Rust build that embeds it. A clean checkout needs the frontend dependencies and bundle before Cargo can build `ignitify-api`:

```sh
cd frontend
pnpm install --frozen-lockfile
pnpm run build
cd ..
cargo check --workspace
```

Run frontend commands from `frontend/`:

```sh
pnpm run check
pnpm run build
pnpm run test
pnpm run test:e2e
```

Use pnpm `11.18.0` and the provided Vite+ (`vp`) scripts. Do not replace them with npm, yarn, or raw Vite invocations. Playwright scripts exist, but no tracked Playwright configuration or authored end-to-end suite currently exists; do not report E2E coverage until one is added.

For local runtime work explicitly requested by the user, copy the relevant values from `.env.example`. Runtime secrets are created under `IGNITIFY_DATA_DIR` on first start when overrides are absent. Never commit `.env`, databases, runtime secrets, generated Traefik certificates, Compose stages, or build workspaces.

## Rust Conventions

### Structure And Visibility

- Keep `lib.rs` and `main.rs` limited to module declarations, public facade/re-exports, documentation, entrypoints, and dependency composition. Do not add HTTP implementation to `main.rs` or broad domain implementation to crate roots.
- Default to private visibility. Use `pub(super)` for immediate-parent helpers, `pub(crate)` for internal crate collaboration, and bare `pub` only for an intentional workspace/public contract.
- Use facade re-exports to expose shallow crate APIs while keeping implementation modules domain-focused.
- Prefer one focused error type and `pub type Result<T>` at a crate or bounded-domain boundary. Use `thiserror` in library crates. Preserve sources internally and expose stable, non-sensitive responses only from `ignitify-api`.
- Use traits at control-plane boundaries (`ImageRuntime`, `Ingress`, `SourceBuild`, DNS and metric providers) instead of binding worker logic directly to Docker, Compose, SSH, or SQL details.
- Several existing Rust modules exceed the preferred size guideline. Do not make a large module larger for new work; split the new cohesive responsibility into child modules and re-export it through the existing facade. Keep new production files below 800 LOC where practical and move inline tests beyond 150 LOC to a sibling `tests.rs` or crate-level tests directory.

### Persistence, Worker, And Errors

- Add durable schema changes as a new, sequential SQL migration in `crates/ignitify-db/migrations/`; never edit an applied migration or reuse an existing migration number. The next committed migration must follow the current `0026` high-water mark. Add repository coverage with an isolated `sqlite::memory:` database when behavior changes.
- Keep SQL in repositories and bind every user-controlled value with SQLx `.bind(...)`. Do not concatenate input into SQL. Use transactions for state transitions that must be atomic, especially deployment, token-family, secret, and audit writes.
- API handlers must map validation to `400`, unauthenticated to `401`, forbidden to `403`, inaccessible/nonexistent to `404`, conflicts to `409`, unavailable capability/dependency to the appropriate `5xx`, and unexpected failures to a non-sensitive `500` response. Only `ignitify-api` implements `IntoResponse`.
- Use `Result` and `?` for recoverable failures. Never use `unwrap()` or `expect()` in production paths. Tests may use them when failure diagnostics remain clear.
- Service/deployment handlers submit state through `ServiceControl` or `ControlHandle`; reconciliation, status observation, route changes, and retry logic belong in the control-plane worker and adapters. Preserve deployment event/log ordering and idempotency behavior.
- Do not bypass encrypted variable/credential repositories. Project/service variables, provider credentials, remote-server credentials, builder TLS material, certificate keys, and backup credentials must remain encrypted with the runtime age identity and must never be returned in plaintext DTOs.

### Security

- Access tokens remain memory-only in the frontend. Refresh tokens remain hashed in SQLite and are delivered only in the `HttpOnly`, `SameSite=Strict` refresh cookie scoped to `/api/v1/auth`.
- Every state-changing cookie route must retain `X-Ignitify-Request` protection and trusted-origin validation. Preserve secure-cookie and HTTPS-origin requirements in remote mode; trust forwarded headers only when the explicit setting enables it.
- Keep bootstrap secret checks, login rate limiting, step-up requirements, authorization checks, audit records, request IDs, terminal concurrency limits, upload limits, and CSP/security headers intact when changing nearby code.
- Client-side route guards are only UX. Every server-side read or mutation must enforce role/membership independently. Admin/operator-only runtime, provider, terminal, backup, remote-builder, remote-server, and infrastructure actions require explicit backend authorization.
- Do not log secrets, access tokens, refresh tokens, private keys, certificate material, OAuth codes, signed URLs, raw deployment environment values, or unredacted command output.

## Vue And TypeScript Conventions

- Use Vue 3 Composition API only: `<script setup lang="ts">` and named composables such as `useDeployment`, `useRuntimeContainers`, or `useControlPlanePreferences`.
- Use double quotes and semicolons. Import app code through `@/`, for example `@/lib/api`.
- Put HTTP calls in `src/lib/api/<domain>.ts`, shared request/refresh behavior in `src/lib/api/core.ts` and `session.ts`, and application state in Pinia setup stores. Do not call `fetch` directly from a view when an API module can own the contract.
- Use `shallowRef` for API records and `computed` for derived state. Keep views orchestration-focused; extract reusable domain behavior into composables and repeated UI into feature components.
- Keep the API contract in sync across the Rust handler DTO, typed API module, composable/store, and UI. For async control-plane actions, implement loading, empty, error, disabled, and success states.
- Reuse shadcn-vue primitives from `@/components/ui/*`; merge classes with `cn()` and define variants with CVA. Use icons from `@lucide/vue`, never `lucide-vue-next`. Give unfamiliar icon-only controls a tooltip and accessible name.
- Use `vue-i18n` keys for user-visible copy and update both `src/i18n/locales/en.ts` and `src/i18n/locales/id.ts` together.
- Tailwind semantic classes must follow runtime tokens: `bg-background`, `text-foreground`, `bg-card`, `border-border`, and related semantic utilities. Do not hard-code semantic palette values unless rendering a chart, terminal output, or explicit state visual. Toggle color mode through the `.dark` class.
- Follow `frontend/design.md`: preserve the compact operational UI, 8px spacing rhythm, stable control dimensions, explicit readable statuses, and accessible keyboard semantics. Do not introduce gradients, heavy shadows, decorative page cards, glass effects, or nonessential motion.

## Important Files

- `Cargo.toml` - workspace members and shared Rust dependencies.
- `.env.example` - documented runtime configuration and security-sensitive defaults; never put real values here.
- `crates/ignitify-core/src/main.rs` - process entrypoint and runtime composition; `operations.rs` owns backup/restore dispatch.
- `crates/ignitify-api/src/lib.rs`, `routes.rs`, `state.rs`, and `handlers/` - public router composition, route registration, dependencies, and HTTP adapters.
- `crates/ignitify-api/build.rs` and `src/frontend.rs` - embed and serve the frontend bundle. Do not hand-edit generated frontend asset code.
- `crates/ignitify-control-plane/src/lib.rs` - worker/control contracts and encrypted service configuration; split new responsibilities from this large facade instead of extending it indiscriminately.
- `crates/ignitify-db/src/database.rs`, `repositories/`, and `migrations/` - SQLite lifecycle, persistence API, and durable schema history.
- `frontend/vite.config.ts` - Vite+, Tailwind, aliases, ports, proxy, lint, and test configuration.
- `frontend/src/router/index.ts` - lazy routes and auth/operator guards.
- `frontend/src/lib/api/core.ts` and `session.ts` - request IDs, Bearer transport, refresh, and same-origin state-change protection.
- `frontend/src/assets/styles/global.css` and `frontend/design.md` - visual tokens and UI contract.
- `infra/operations/README.md`, `infra/git-build/README.md`, `infra/remote-builder/README.md`, and `infra/traefik/README.md` - operational constraints for backup, builds, and ingress.

## Testing And QA

- For Rust changes, run the focused crate/package test first, then run `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings`. Run `cargo check --workspace` when a full test run is not proportionate or before the broader gate.
- For frontend changes, run `pnpm run check` and `pnpm run build`; run `pnpm run test` for API modules, composables, stores, and components with behavior changes. Frontend builds refresh the ignored `frontend/dist` bundle required by Rust embedding.
- Add a focused regression test for non-trivial authentication, authorization, encryption, persistence, deployment policy/state, runtime safety, stream, terminal, monitoring, or infrastructure behavior.
- Backend tests live beside source under `#[cfg(test)]` or in the existing crate test modules. Use isolated in-memory SQLite where appropriate. Frontend specs use Vitest with happy-dom and live under `frontend/src/` as `*.spec.ts` or `*.test.ts`.
- Do not mutate real `data/`, a configured runtime, remote server, provider, S3 destination, DNS, Docker daemon, or Traefik stack to validate a code change unless the user explicitly authorizes that exact operation.
