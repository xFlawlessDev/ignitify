# Contributing To Ignitify

Ignitify controls deployment hosts and handles sensitive infrastructure
credentials. Contributions must favor explicit authorization, narrow scope,
safe failure behavior, and regression coverage over convenience.

## Before You Start

- Read [AGENTS.md](AGENTS.md). It defines module ownership, dependency
  direction, security boundaries, code conventions, and required validation.
- Read the relevant operational guide under `infra/` before changing Git
  builds, Traefik, remote builders, backups, or restore behavior.
- Search for established repository patterns before introducing a dependency,
  abstraction, or a new configuration setting.
- Keep feature work focused. Separate unrelated refactors from behavioral
  changes so that review and rollback remain practical.

## Development Setup

Install the Rust toolchain, Node.js, and pnpm `11.18.0`. The backend embeds the
frontend production bundle, so build the frontend before Rust builds that
compile `ignitify-api`:

```sh
cd frontend
pnpm install --frozen-lockfile
pnpm run build
cd ..
cargo check --workspace
```

For local runtime work, create an untracked `.env` from `.env.example` and set
an `IGNITIFY_BOOTSTRAP_SECRET` of at least 32 random bytes. For Vite's local
HTTP session, set `IGNITIFY_SECURE_COOKIES=false` and add the localhost port
`6565` origins as documented in `.env.example`.

Never commit `.env`, databases, runtime secret files, generated certificates,
provider credentials, private keys, deployment environment values, backup
snapshots, or production logs.

## Implementation Guidelines

- Keep external Docker, Compose, SSH, Git, DNS, ingress, monitoring, and S3
  effects in workers and runtime adapters. HTTP handlers validate, authorize,
  submit/read state, and map errors only.
- Keep authentication, authorization, step-up, origin checks, cookie
  protections, audit records, encryption, and secret redaction intact when
  working nearby.
- Add database changes through a new sequential migration. Never change an
  applied migration, reuse a migration number, or construct SQL with
  user-controlled strings.
- Add focused regression coverage for non-trivial changes to authentication,
  authorization, persistence, encryption, deployment policy/state, runtime
  safety, streams, terminals, monitoring, or infrastructure behavior.
- Keep the frontend API contract synchronized across Rust DTOs, typed API
  modules, composables/stores, and views. Implement loading, empty, error,
  disabled, and success states for asynchronous controls.

## Validate Your Change

Run the focused check for the changed surface first. For Rust changes, run the
full quality gate from the repository root:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

For frontend changes, run these commands from `frontend/`:

```sh
pnpm run check
pnpm run build
pnpm run test
```

End-to-end scripts exist, but this repository does not currently track a
Playwright configuration or authored end-to-end suite. Do not claim E2E
coverage until one is added.

Do not run Docker, Docker Compose, SSH, DNS, provider, backup, restore, or
other external operations merely to validate a contribution unless the exact
target and authorization are explicit.

## Pull Requests

Create a focused branch from the current mainline and submit one coherent
behavioral change per pull request. A pull request description should include:

- what changed and why;
- validation run and its result;
- migrations, configuration, security, or operator impact;
- any follow-up work intentionally left out of scope.

Do not include passwords, access or refresh tokens, provider credentials, SSH
keys, certificate material, S3 credentials, database snapshots, or production
logs in commits, issue reports, screenshots, or pull requests.

## Security Reports

Do not disclose a suspected vulnerability or sensitive operational detail in a
public issue. Contact the project maintainer through a private channel and
include enough information to reproduce the problem safely.

## License

By submitting a contribution, you agree that it may be distributed under the
project's dual license: [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at the recipient's option.
