# Repository Guidelines

## Project Overview

Ignitify is a self-hosted deployment control plane built with Rust and Vue 3. Current product slice delivers local SQLite-backed password auth; project and deployment views are UI fixtures until control-plane work lands.

## Architecture & Data Flow

- Rust workspace uses Rust 2024 and shared dependencies from `Cargo.toml`.
- `ignitify-core` is composition root and Axum HTTP adapter. Keep handlers small: extract, authenticate, validate, call service/repository, map error.
- `ignitify-auth` owns Argon2 credentials, JWT access tokens, rotating hashed refresh tokens, and auth DTOs. It receives `Database` through `AuthService::new(database, config)`.
- `ignitify-db` owns SQLite connection setup, embedded migrations, repositories, and database records. Add numbered SQL migrations, for example `crates/ignitify-db/migrations/0002_projects.sql`.
- Frontend flow: `src/main.ts` installs Pinia and Router; router guard initializes `useAuthStore`; API client adds in-memory Bearer token; refresh uses HttpOnly cookie; Vite proxies `/api` to backend.
- Backend runs `127.0.0.1:5656`; frontend runs `6565`.

Keep future deployment handlers separate from Docker or ingress execution. HTTP should enqueue/read state; worker/control-plane code should own external effects and retries.

## Key Directories

- `crates/ignitify-core/` - Axum startup, routes, cookies, HTTP error mapping.
- `crates/ignitify-auth/` - auth service and session/JWT behavior.
- `crates/ignitify-db/` - SQLx SQLite access, migrations, repository records.
- `frontend/src/lib/api/` - typed API functions and token/refresh transport.
- `frontend/src/stores/` - Pinia domain state; `auth.ts` is setup-store pattern.
- `frontend/src/views/` - routed page surfaces, for example `ProjectsView.vue`.
- `frontend/src/components/` - application components; reuse primitives in `components/ui/`.
- `frontend/src/assets/styles/global.css` - Tailwind v4 imports and runtime design tokens.
- `frontend/design.md` - UI contract: theme, spacing, semantic color, layout rules.
- `thoughts/shared/` - research and design artifacts; not runtime source.

## Development Commands

Run Rust commands from repository root:

```sh
cargo run -p ignitify-core
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Backend requires local `.env` values from `.env.example`:

```sh
IGNITIFY_JWT_SECRET="at-least-32-random-characters" cargo run -p ignitify-core
curl http://127.0.0.1:5656/health
```

Run frontend commands from `frontend/`:

```sh
pnpm install
pnpm dev
pnpm check
pnpm build
pnpm test
pnpm test:e2e
```

`pnpm dev` serves `http://127.0.0.1:6565` and proxies `/api` to port `5656`.

## Code Conventions & Common Patterns

### Rust

- Format with `cargo fmt`; keep Clippy clean with warnings denied.
- Use `snake_case` functions/modules, `PascalCase` types, singular error enums such as `AuthError` and `DatabaseError`.
- Prefer explicit local aliases: `pub type Result<T> = std::result::Result<T, AuthError>`.
- Use `thiserror` enums and map internal database/crypto/JWT errors at HTTP boundary. Do not expose server internals in JSON errors.
- Keep async I/O methods `async`; use SQLx `.bind(...)`, never concatenate user input into SQL.
- Keep dependencies directional: `core -> auth/db`; auth can use db; db never imports HTTP/auth crates. Pass dependencies through constructors, for example `AuthService::new(database, config)`.
- Store refresh tokens as hashes only. Keep access tokens memory-only client side. State-changing cookie routes must retain `X-Ignitify-Request` protection.

### Vue and TypeScript

- Use Vue 3 Composition API only: `<script setup lang="ts">`, named composables such as `useControlPlanePreferences`.
- Use double quotes and semicolons. Import app code through `@/`, for example `@/lib/api`.
- Put HTTP calls in `src/lib/api/<domain>.ts`, shared request behavior in `src/lib/api/core.ts`, and application state in Pinia setup stores.
- Use `shallowRef` for API records and `computed` for derived UI state. Keep views orchestration-focused; extract reusable sections to `components/` and domain behavior to `composables/`.
- Reuse shadcn-vue primitives from `@/components/ui/*`; merge classes with `cn()`; define variants with CVA. Use `@lucide/vue`, never `lucide-vue-next`.
- Tailwind semantic classes must follow runtime tokens: `bg-background`, `text-foreground`, `bg-card`, `border-border`. Do not hard-code palette values unless rendering explicit status/chart visuals. Toggle color mode through the `.dark` class.
- Preserve compact control-plane UI: no gradients, heavy shadows, or decorative cards; keep keyboard/accessibility semantics on controls.

## Important Files

- `Cargo.toml` - workspace members and shared Rust dependencies.
- `.env.example` - backend runtime variables; never commit real secrets.
- `crates/ignitify-core/src/main.rs` - API entrypoint, route registration, auth/cookie adapter.
- `crates/ignitify-auth/src/lib.rs` - public auth contract and session lifecycle.
- `crates/ignitify-db/src/lib.rs` - database configuration, migration bootstrapping, repositories.
- `crates/ignitify-db/migrations/0001_auth.sql` - current durable auth schema.
- `frontend/vite.config.ts` - Vite+, Tailwind plugin, aliases, ports, API proxy, test inclusion.
- `frontend/src/router/index.ts` - lazy routes and auth guard.
- `frontend/src/lib/api/core.ts` - API result/error/refresh handling.
- `frontend/src/assets/styles/global.css` and `frontend/design.md` - UI token and visual contract.

## Runtime/Tooling Preferences

- Use Rust 2024 and Cargo workspace dependencies. Add a dependency only when standard library/current dependencies cannot cover job.
- Use pnpm `11.18.0` and Vite+ (`vp` scripts). Do not replace with npm/yarn or raw Vite commands.
- Use Vue 3, Pinia, Vue Router, Tailwind CSS v4, shadcn-vue/Reka UI, and `@lucide/vue`.
- SQLite is current control-plane store. Do not introduce Postgres, Redis, a job broker, Docker SDK, or ingress tooling without an approved domain slice.
- Never run Docker commands, bind public ports, or touch production resources unless task explicitly requires it.

## Testing & QA

- Run focused checks for changed surface, then `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` for Rust changes.
- Run `pnpm check` and `pnpm build` for frontend changes. Run `pnpm test` for composable/store/API behavior. E2E commands require a Playwright configuration and tests; current repository has no authored E2E suite.
- Backend tests live beside source under `#[cfg(test)]`; use isolated `sqlite::memory:` databases. Example: refresh-token reuse must revoke its family.
- Frontend specs use Vitest and `happy-dom`, named `*.spec.ts` under `frontend/src/`. Example: `useControlPlanePreferences.spec.ts` verifies localStorage plus DOM theme state.
- Add one focused regression test for non-trivial auth, persistence, state, or policy change. Current dashboard/project data is static UI fixture; do not claim deployment behavior without backend coverage.
