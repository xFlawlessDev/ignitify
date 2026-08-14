# Deployment lifecycle

## Resource model

A project has a default environment and many services. A service has runtime configuration, variables, optional source configuration, and a desired generation/state. A deployment is an immutable snapshot of the service and environment when the request is accepted.

```text
Project environment
       +
Service variables
       |
       v
encrypted deployment snapshot -> queue -> worker -> runtime + ingress
```

Project environment values are merged first, then service variables with the same key override them. The deployment snapshot is encrypted before it is stored. Later environment changes do not alter a submitted deployment.

## Service kinds

| Kind      | Configuration                                                                                       |
| --------- | --------------------------------------------------------------------------------------------------- |
| `image`   | OCI reference pinned to a SHA-256 digest, with an optional internal port and exec-form healthcheck. |
| `compose` | Compose YAML, exposed service name, and an optional internal port.                                  |

Service names and exposed Compose services must be lower-case DNS labels. Compose YAML is limited to 1 MiB and validated by the runtime before it runs. Image tags such as `nginx:latest` are rejected; use `image@sha256:<64-hex>`.

## Source configuration

`source_config` separates source origin from runtime specification.

| `source`      | Requirements                                                              |
| ------------- | ------------------------------------------------------------------------- |
| `template`    | `template` is required.                                                   |
| `compose`     | May use local YAML or a repository/provider with a repository and branch. |
| `application` | `provider_id`, `repository`, `branch`, and `builder` are required.        |

Available application builders are `dockerfile`, `static`, `spa`, and `railpack`. The Git executor resolves a branch to a specific commit and stores that revision on the deployment. Rollback uses the stored revision, not the current branch tip.

## Deployment state

```text
queued -> preparing -> running -> healthy
                  \-> failed
running/healthy -> stopping -> stopped
healthy -> superseded
```

The worker only permits valid transitions. Deploy requests use a visible-ASCII idempotency key between 1 and 128 bytes, so a client can retry without creating duplicate deployments.

## Domains and ingress

A domain must be a complete lower-case ASCII hostname, not an IP, `localhost`, wildcard, or public suffix. It starts as `pending`, becomes `active` when the route is applied, and becomes `failed` when route reconciliation fails.

Traefik only discovers containers with the `com.ignitify.managed=true` label. Services with a domain join the `ignitify-proxy` network; the worker manages their routes and labels.

## Events, logs, stop, and rollback

- `POST /api/v1/services/{service_id}/deployments` queues a deployment.
- `GET /api/v1/deployments/{deployment_id}/events` and `/logs` serve resumable SSE streams.
- `POST /api/v1/services/{service_id}/stop` requests a stop lifecycle.
- `POST /api/v1/deployments/{deployment_id}/rollback` queues a deployment from an earlier deployment snapshot/revision.

The worker redacts logs that could contain snapshot values. Do not use build or application output as a channel for exposing secrets.
