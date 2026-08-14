# Control plane API

Local base URL: `http://127.0.0.1:5656`. All control plane routes are under `/api/v1`, except `GET /health`.

## Conventions

- Requests and responses use JSON except for multipart uploads, SSE, and the terminal WebSocket.
- Protected routes require `Authorization: Bearer <access-token>`.
- State-changing requests also require `X-Ignitify-Request: 1` and an `Origin` listed in `IGNITIFY_TRUSTED_ORIGINS`.
- Error responses do not expose database, token, or runtime details. Statuses `400`, `401`, `403`, `404`, `409`, and `500` represent the general failure classes.
- Resource identifiers are UUIDs. Use an idempotency key when creating a deployment.

## Authentication

| Method | Path                     | Auth   | Description                                            |
| ------ | ------------------------ | ------ | ------------------------------------------------------ |
| `GET`  | `/health`                | None   | Basic process probe.                                   |
| `GET`  | `/api/v1/auth/bootstrap` | None   | Check whether the first admin is required.             |
| `POST` | `/api/v1/auth/bootstrap` | None   | Create the first admin; only once.                     |
| `POST` | `/api/v1/auth/login`     | None   | Create a session and refresh cookie.                   |
| `POST` | `/api/v1/auth/refresh`   | Cookie | Rotate the refresh token and issue a new access token. |
| `POST` | `/api/v1/auth/logout`    | Cookie | Revoke the refresh session and remove the cookie.      |
| `GET`  | `/api/v1/auth/me`        | Yes    | Return the current user.                               |

Bootstrap/login bodies use `{ "username": "...", "password": "..." }`. The session response contains `access_token`, `token_type`, `expires_at`, and `user`; the refresh cookie is sent with `Set-Cookie`.

## Dashboard and providers

| Method            | Path                                           | Description                                             |
| ----------------- | ---------------------------------------------- | ------------------------------------------------------- |
| `GET`             | `/api/v1/dashboard`                            | Project, service, and deployment summary for the actor. |
| `GET`, `POST`     | `/api/v1/providers`                            | List or create a source provider.                       |
| `POST`            | `/api/v1/providers/github/manifest`            | Start the GitHub App manifest flow.                     |
| `GET`             | `/api/v1/providers/github/manifest/callback`   | GitHub manifest callback.                               |
| `PATCH`, `DELETE` | `/api/v1/providers/{provider_id}`              | Update or delete a provider.                            |
| `POST`            | `/api/v1/providers/{provider_id}/test`         | Test a provider connection.                             |
| `GET`             | `/api/v1/providers/{provider_id}/repositories` | List accessible repositories.                           |
| `GET`             | `/api/v1/providers/{provider_id}/branches`     | List branches for a provider repository.                |

Provider credentials are encrypted by the backend. The GitHub App manifest flow and provider discovery do not mean every provider credential can be used by the build executor; the Git executor does not currently support GitHub App credentials.

## Runtime and terminal

| Method   | Path                                                 | Access | Description                                                   |
| -------- | ---------------------------------------------------- | ------ | ------------------------------------------------------------- |
| `GET`    | `/api/v1/runtime/status`                             | User   | Database/runtime/worker/ingress readiness and metric summary. |
| `GET`    | `/api/v1/runtime/metrics`                            | User   | CPU, memory, disk, network, and container metrics.            |
| `GET`    | `/api/v1/runtime/containers`                         | User   | Container inventory when the runtime is available.            |
| `GET`    | `/api/v1/runtime/containers/{container_id}/details`  | Admin  | Configuration, mounts, network, and label details.            |
| `GET`    | `/api/v1/runtime/containers/{container_id}/logs`     | Admin  | Container logs.                                               |
| `POST`   | `/api/v1/runtime/containers/{container_id}/upload`   | Admin  | Multipart upload, maximum 8 MiB.                              |
| `DELETE` | `/api/v1/runtime/containers/{container_id}`          | Admin  | Delete a container.                                           |
| `GET`    | `/api/v1/terminal`                                   | Admin  | Upgrade to the host terminal WebSocket.                       |
| `GET`    | `/api/v1/runtime/containers/{container_id}/terminal` | Admin  | Upgrade to the container terminal WebSocket.                  |

Uploads accept the multipart `file` field and an optional `destination` (default `/tmp`). The terminal authenticates and validates the WebSocket origin; use the dashboard client as the protocol reference.

## Projects, environments, and services

| Method                   | Path                                        | Description                                      |
| ------------------------ | ------------------------------------------- | ------------------------------------------------ |
| `GET`, `POST`            | `/api/v1/projects`                          | List accessible projects or create a project.    |
| `GET`, `PATCH`           | `/api/v1/projects/{project_id}`             | Read or update a project.                        |
| `GET`, `PUT`             | `/api/v1/projects/{project_id}/environment` | Read or replace the project default environment. |
| `GET`                    | `/api/v1/projects/{project_id}/deployments` | Deployments for every service in the project.    |
| `GET`                    | `/api/v1/projects/{project_id}/activity`    | Project activity.                                |
| `GET`, `POST`            | `/api/v1/projects/{project_id}/services`    | List or create a service.                        |
| `GET`, `PATCH`, `DELETE` | `/api/v1/services/{service_id}`             | Read, update, or delete a service.               |
| `GET`, `POST`            | `/api/v1/services/{service_id}/domains`     | List or add a domain.                            |
| `DELETE`                 | `/api/v1/domains/{domain_id}`               | Delete a domain with confirmation.               |

Example image service request:

```json
{
  "name": "web",
  "kind": "image",
  "image_reference": "nginx@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "internal_port": 8080,
  "healthcheck": ["/bin/sh", "-c", "wget -qO- http://localhost:8080/health"],
  "variables": [
    { "key": "LOG_LEVEL", "value": "info", "is_secret": false },
    { "key": "DATABASE_URL", "value": "secret", "is_secret": true }
  ]
}
```

For a Compose service, use `kind: "compose"`, `compose_yaml`, `exposed_service`, `internal_port`, and `variables`. `source_config` can be added for a template, Git Compose, or application source. Secret values are not returned as plaintext.

Project environments use this shape. To keep an existing secret, send `value: null` with `is_secret: true`; a new value is required for non-secret variables.

```json
{
  "variables": [
    { "key": "REGION", "value": "id", "is_secret": false },
    { "key": "API_KEY", "value": null, "is_secret": true }
  ]
}
```

## Deployments and streams

| Method        | Path                                           | Description                                      |
| ------------- | ---------------------------------------------- | ------------------------------------------------ |
| `GET`, `POST` | `/api/v1/services/{service_id}/deployments`    | List service deployments or submit a deployment. |
| `POST`        | `/api/v1/services/{service_id}/stop`           | Request a service stop.                          |
| `GET`         | `/api/v1/deployments/{deployment_id}`          | Deployment details.                              |
| `POST`        | `/api/v1/deployments/{deployment_id}/rollback` | Submit a rollback from a deployment snapshot.    |
| `GET`         | `/api/v1/deployments/{deployment_id}/events`   | Deployment lifecycle SSE events.                 |
| `GET`         | `/api/v1/deployments/{deployment_id}/logs`     | SSE log lines.                                   |

Deployment submission requires an idempotency key header according to the client contract. To resume SSE, send `Last-Event-ID` or the `after=<sequence>` query parameter. Streams send a `snapshot` event when the cursor is older than the retained data, a heartbeat about every 15 seconds, and `log` events on the log stream.

## Contract sources

The routes above come from `ignitify/crates/ignitify-api/src/routes.rs`. Response DTOs and specific validation live in `handlers/` and `ignitify-domain`. When changing a contract, update the route, typed dashboard client, tests, and this page in one change.
