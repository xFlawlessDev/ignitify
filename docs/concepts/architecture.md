# Architecture

Ignitify separates HTTP, domain, persistence, and external side effects. This keeps handlers thin, makes validation testable without a Docker runtime, and limits where secrets or host access are available.

## Runtime components

```text
Browser dashboard
  -> Vue Router / Pinia / typed API client
  -> Vite proxy during development
  -> Axum API
       -> AuthService + SQLite repositories
       -> ServiceControl / ControlHandle
       -> deployment worker
            -> Docker or Compose runtime
            -> Git source builder
            -> Traefik ingress
```

`ignitify-core` reads configuration, creates runtime secrets when needed, opens SQLite, creates Docker/Compose/Traefik/Git adapters, and starts the worker and Axum router. The default listener is `127.0.0.1:5656`.

## Crates and responsibilities

| Crate                      | Responsibility                                                                           |
| -------------------------- | ---------------------------------------------------------------------------------------- |
| `ignitify-core`            | Composition root, listener, runtime secrets, and dependency readiness.                   |
| `ignitify-api`             | Routes, HTTP DTOs, auth/origin extraction, cookies, SSE streams, and safe error mapping. |
| `ignitify-auth`            | Argon2 passwords, JWT access tokens, rotating refresh tokens, and sessions.              |
| `ignitify-db`              | SQLite pool, embedded migrations, persistence models, and authorized repositories.       |
| `ignitify-domain`          | Identifiers, service specs, deployment/domain status, and input rules.                   |
| `ignitify-control-plane`   | Environment encryption, deployment submission, worker, lifecycle, and event/log streams. |
| `ignitify-runtime-docker`  | Inventory, metrics, container actions, and OCI image runtime.                            |
| `ignitify-runtime-compose` | Compose service validation and lifecycle.                                                |
| `ignitify-ingress-traefik` | Traefik network and labels for service domains.                                          |
| `ignitify-source-git`      | Source checkout, commit resolution, and Dockerfile/static/Railpack builders.             |
| `ignitify-terminal`        | Protected PTY terminals for hosts and containers.                                        |

## Persistent data

SQLite stores users, refresh token hashes, projects and memberships, services, deployments, domains, activity, providers, and source configuration. Migrations live in `ignitify-db/migrations/` and are embedded by the database crate.

Project and service environment values are not stored as plaintext. `ServiceControl` encrypts values with age, using a runtime identity stored separately from the database. When read, secret values remain masked; non-secret values can only be opened by roles allowed to manage the service.

## Authorization

Authentication produces an `AuthenticatedUser` with an `admin` or `user` role. Within a project, membership roles are:

| Role     | Permission                                                |
| -------- | --------------------------------------------------------- |
| `owner`  | Change the project and manage services/environment.       |
| `editor` | Manage services/environment without changing the project. |
| `viewer` | Read accessible resources.                                |
| `admin`  | Cross-project access and protected host operations.       |

Repositories receive the actor and evaluate access before returning data. Handlers must not rely on UI filtering to enforce authorization.

## Observability and health

`GET /health` is an unauthenticated basic probe. `GET /api/v1/runtime/status` checks the database, runtime, worker, ingress, and host metrics as `ready` or `unavailable`. Detailed system metrics are available at `GET /api/v1/runtime/metrics`; Docker inventory is available through the runtime endpoints.

Deployment events and logs use SSE. Streams can replay from a stored cursor (`Last-Event-ID` or `after`) and send a snapshot when the client's cursor is older than the event retention window.
