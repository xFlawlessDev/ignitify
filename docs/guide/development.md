# Development

## Repository map

```text
ignitify/
  crates/
    ignitify-core/            Runtime composition and listener
    ignitify-api/             Axum routes, handlers, DTOs, and HTTP errors
    ignitify-auth/            Passwords, JWT, and refresh sessions
    ignitify-db/              SQLite, migrations, models, and repositories
    ignitify-domain/          Domain types and validation
    ignitify-control-plane/   Queue, worker, snapshots, and streams
    ignitify-runtime-*/       Docker and Compose adapters
    ignitify-ingress-traefik/ Traefik adapter
    ignitify-source-git/      Git checkout and builder
    ignitify-terminal/        Host/container PTY
  frontend/                   Vue 3 administrator dashboard
  infra/                      Traefik stack and executor documentation
```

## Architecture boundaries

Backend dependencies flow in one direction: `core -> api -> auth -> db -> domain`. `ignitify-api` may use contracts from database/domain as an adapter, but lower crates must not import HTTP or runtime code.

- Add endpoints in `ignitify-api/src/routes.rs` and handlers in `handlers/<domain>.rs`.
- Put business validation and I/O-free types in `ignitify-domain`.
- Add data changes as numbered SQL migrations in `ignitify-db/migrations/` and access them through a repository.
- External deployment side effects belong in `ignitify-control-plane` and adapter crates, not HTTP handlers.
- `ignitify-core/src/main.rs` only composes dependencies, workers, and the listener.

## Change workflow

1. Write or update domain validation and its unit tests.
2. Add a migration when the persistence contract changes.
3. Change repositories and the control plane before the HTTP handler.
4. Connect the typed dashboard API client, composable/store, and view.
5. Add focused tests for authorization, state, persistence, or UI regressions.
6. Run the quality gate for the changed area.

## Backend conventions

- Rust 2024, typed error enums per crate, and `Result` with `?` for recoverable failures.
- No `unwrap()` or `expect()` on production paths.
- SQL uses bind parameters; transactions cover writes that must be atomic.
- APIs map errors to safe responses. Do not send SQL details, tokens, database URLs, or runtime errors to clients.
- Split Rust files that grow beyond roughly 800 lines and keep visibility minimal.

## Dashboard conventions

- Use Vue 3 Composition API with `<script setup lang="ts">`.
- Put HTTP in `src/lib/api/<domain>.ts`; put reusable state and orchestration in a composable or Pinia setup store.
- Use semantic Tailwind tokens such as `bg-background`, `text-foreground`, and `border-border`.
- Use components from `src/components/ui/`, `cn()`, and icons from `@lucide/vue`.
- Every data route must go through `apiFetch` so bearer tokens, refresh, timeouts, and state-changing request headers stay consistent.

## Documentation changes

`docs/` is the source content for the public documentation hosted by the
separate Ignitify marketing site. It is intentionally not a VitePress project
in this repository.

Documentation uses file-based paths. Add English Markdown pages under `docs/`
and Indonesian translations at the matching `docs/id/` path. Keep headings,
links, and relative asset references portable so the marketing repository can
consume the content as its VitePress documentation root. Update VitePress
configuration, locale sidebars, navigation, and the documentation build in the
marketing repository.

Do not put tokens, passwords, age identities, or real `.env` contents in documentation, examples, snapshots, or screenshots.
