# Repository Guidelines

## Project Overview

Ignitify is a self-hosted deployment control plane built with Rust and Vue 3. Current product slice delivers local SQLite-backed password auth and projects; deployment control-plane behavior has not landed.

## Architecture & Data Flow

- Rust workspace uses Rust 2024 and shared dependencies from `Cargo.toml`.
- `ignitify-core` is runtime composition root only: read runtime config, build dependencies, bind listener, call `axum::serve`. It owns no routes, handlers, request/response DTOs, or HTTP error mapping.
- `ignitify-api` owns Axum route registration, HTTP handlers, request/response DTOs, cookie/origin helpers, authentication extraction, and safe HTTP error mapping. Handlers extract, authenticate, validate, call a service/repository, then map errors.
- `ignitify-auth` owns Argon2 credentials, JWT access tokens, rotating hashed refresh tokens, auth DTOs, and `AuthError`. It receives `Database` through `AuthService::new(database, config)`.
- `ignitify-db` owns SQLite connection setup, embedded migrations, `models/`, and `repositories/`. Add numbered SQL migrations, for example `crates/ignitify-db/migrations/0002_projects.sql`.
- Frontend flow: `src/main.ts` installs Pinia and Router; router guard initializes `useAuthStore`; API client adds in-memory Bearer token; refresh uses HttpOnly cookie; Vite proxies `/api` to backend.
- Backend runs `127.0.0.1:5656`; frontend runs `6565`.

Keep future deployment handlers separate from Docker or ingress execution. HTTP should enqueue/read state; worker/control-plane code should own external effects and retries.

## Key Directories

- `crates/ignitify-core/` - runtime config, dependency composition, listener, process error.
- `crates/ignitify-api/` - Axum routes, handlers, HTTP DTOs, cookies, origin/auth extraction, API error mapping.
- `crates/ignitify-auth/` - auth service and session/JWT behavior.
- `crates/ignitify-db/` - SQLx SQLite access, migrations, models, and repositories.
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
pnpm run dev
pnpm run check
pnpm run build
pnpm run test
pnpm run test:e2e
```

`pnpm run dev` serves `http://127.0.0.1:6565` and proxies `/api` to port `5656`.

## Code Conventions & Common Patterns

### Rust

#### Rust Architecture & Modularity Rules

1. **File Size Cap (500–800 LOC Rule):**
   - No single `.rs` file may exceed 800 lines of code.
   - When a file grows beyond this, convert `file.rs` into `file/mod.rs` (or `file.rs` plus sibling submodules) and split `impl` blocks or domain logic into separate child submodules.

2. **Clean Library Roots:**
   - `lib.rs` and `main.rs` must contain only module declarations (`mod foo;`), `pub use` re-exports, top-level documentation, and entrypoints.
   - Never write heavy implementation logic or global state directly inside `lib.rs`.

3. **Strict Minimum Visibility:**
   - Default to private (`fn` or `struct`).
   - Use `pub(super)` for submodule helpers shared only with immediate parent modules.
   - Use `pub(crate)` for items shared within the crate.
   - Reserve bare `pub` exclusively for items intended as part of the public crate/workspace API.

4. **The Facade Pattern (`pub use`):**
   - Keep internal file structures deep and domain-focused, but expose a shallow public API at the module root via `pub use`.

5. **Split Large `impl` Blocks:**
   - Do not write massive single `impl MyStruct` blocks.
   - Group related methods into logical submodules across multiple files using separate `impl MyStruct` blocks.

6. **Decouple via Traits:**
   - Do not tie business modules directly to concrete database repositories, network drivers, or heavy external types.
   - Accept traits or generics (`R: TaskRepository` / `dyn TaskRepository`) to allow fast parallel compilation.

7. **Test File Hygiene:**
   - Keep inline `#[cfg(test)]` modules under 150 lines.
   - If tests exceed 150 lines, move them to a sibling `tests.rs` file within the module folder (`#[cfg(test)] mod tests;`) or into the crate-level `tests/` directory.

8. **Scoped Error Enums:**
   - Prefer scoped domain errors (for example, `task_coordinator::Error`) over giant crate-wide error enums.
   - Use `thiserror` for library crates.

#### Crate Structure

- Keep dependency direction one-way: `ignitify-core -> ignitify-api -> ignitify-auth -> ignitify-db -> ignitify-domain`. `ignitify-api` may also depend on `ignitify-db` and `ignitify-domain` for adapter contracts. No lower crate imports `ignitify-api` or `ignitify-core`.
- `ignitify-core/src/main.rs` stays runtime-only. Do not add handlers, DTOs, routes, request extractors, cookie helpers, or `IntoResponse` implementations there.
- In `ignitify-api`, keep `routes.rs` for route registration, `handlers/<resource>.rs` for resource handlers, `extract.rs` for shared HTTP extraction/response helpers, `state.rs` private, and `error.rs` for `ApiError`.
- In `ignitify-db`, keep pool/migration composition in `database.rs`; public persistence records in `models/`; SQL and repository methods in `repositories/<resource>.rs`; private SQL row structs beside their repository. Re-export deliberate public API from `lib.rs` only.
- Keep domain validation and domain types in `ignitify-domain`; never put SQLx, Axum, auth, or runtime types there. Split by bounded domain ownership, not arbitrary file length.
- Keep modules private by default. Use `pub(crate)` for same-crate routing; expose only contracts used across crates.

#### Errors And Safety

- Every crate defines one typed error enum in `error.rs` or crate root: `CoreError`, `ApiError`, `AuthError`, `DatabaseError`, or domain `InputError`. Define `pub type Result<T> = std::result::Result<T, ErrorType>` for fallible crate APIs.
- Use `thiserror` for library/crate errors. Use `#[from]` only at an ownership boundary. Preserve sources internally; map to stable, non-sensitive messages at API boundary.
- Only `ignitify-api` implements `IntoResponse`. Never return `sqlx::Error`, JWT errors, password errors, database URLs, SQL, or internal details to clients.
- Use `Result` and `?` for recoverable failures. No `unwrap()` or `expect()` in production paths. Tests may use `unwrap()`.
- Map input validation to `400`, unauthenticated to `401`, forbidden to `403`, inaccessible/nonexistent to `404`, conflicts to `409`, and unexpected failures to `500`.
- Keep async I/O methods `async`; use SQLx `.bind(...)`, never concatenate user input into SQL. Transactions must cover all writes needing atomicity.
- Store refresh tokens as hashes only. Keep access tokens memory-only client side. State-changing cookie routes must retain `X-Ignitify-Request` protection and trusted-origin validation.

#### Quality Gate

- Format with `cargo fmt`; keep Clippy clean with warnings denied. Run `cargo check --workspace`, `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` for Rust changes.
- Add focused regression coverage for non-trivial auth, persistence, authorization, state, or policy changes. Name tests after behavior.

#### References

- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- Rust modules and visibility: https://doc.rust-lang.org/book/ch07-03-paths-for-referring-to-an-item-in-the-module-tree.html
- Rust recoverable errors and `?`: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html
- Axum error handling: https://docs.rs/axum/latest/axum/error_handling/
- `thiserror`: https://docs.rs/thiserror/latest/thiserror/
- Rust API Guidelines, failure docs: https://rust-lang.github.io/api-guidelines/documentation.html#function-docs-include-error-panic-and-safety-considerations-c-failure

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
- `crates/ignitify-core/src/main.rs` - runtime entrypoint: config, dependency composition, listener, server.
- `crates/ignitify-api/src/lib.rs` and `routes.rs` - public API router and route registration; `handlers/` owns HTTP adapters.
- `crates/ignitify-auth/src/lib.rs` - public auth contract and session lifecycle.
- `crates/ignitify-db/src/lib.rs` - persistence public facade; `database.rs`, `models/`, and `repositories/` own implementation.
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
