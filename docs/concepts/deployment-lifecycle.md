# Deployment lifecycle

## Resource model

A project has a default environment and many services. A service has runtime configuration, variables, optional source configuration, and a desired generation/state. A deployment is an immutable snapshot of the service and environment when the request is accepted.

```text
Project environment
       +
Service variables
       |
       v
encrypted deployment snapshot -> approval -> queue -> worker -> runtime + ingress
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

## Production promotion and approval

The default project environment is `production`. A request for a production
deployment or rollback creates an immutable snapshot with approval status
`pending`; it is not claimable by the worker. A project owner or platform
operator must call `POST /api/v1/deployments/{deployment_id}/approve` before
the snapshot is queued for execution. Editors can request a deployment but
cannot approve it. A single owner may approve their own request so a
single-maintainer installation remains operable, but the request and approval
remain separate, audited actions.

The history keeps the source revision and image digest associated with the
snapshot. Direct images already carry a required immutable digest. Git builds
record their resolved commit and local image digest before runtime start; a
rollback reuses its snapshot revision rather than the branch tip. API responses
expose these values under `source_identity` when known. Pending approval can be
cancelled and never triggers Docker, Compose, SSH, Git build, or ingress work.

## Domains and ingress

A domain must be a complete lower-case ASCII hostname, not an IP, `localhost`, wildcard, or public suffix. It starts as `pending`, becomes `active` when the route is applied, and becomes `failed` when route reconciliation fails.

Traefik only discovers containers with the `com.ignitify.managed=true` label. Services with a domain join the `ignitify-proxy` network; the worker manages their routes and labels.

## Events, logs, stop, and rollback

- `POST /api/v1/services/{service_id}/deployments` requests a production deployment.
- `POST /api/v1/deployments/{deployment_id}/approve` records production approval and queues it.
- `GET /api/v1/deployments/{deployment_id}/events` and `/logs` serve resumable SSE streams.
- `POST /api/v1/services/{service_id}/stop` requests a stop lifecycle.
- `POST /api/v1/deployments/{deployment_id}/rollback` queues a deployment from an earlier deployment snapshot/revision.

The worker redacts logs that could contain snapshot values. Do not use build or application output as a channel for exposing secrets.

## Supply-chain policy

Every new deployment stores a supply-chain report before worker execution. It
records provenance, SBOM, and vulnerability-policy checks. Direct image
deployments pass provenance only when their immutable image digest is present;
source builds replace the initial report after both the resolved source revision
and built image digest are known.

Platform operators configure the delivery policy as either `warning` (the
default) or `require-provenance`. The worker evaluates the current policy after
source resolution and before `runtime.start`. `require-provenance` blocks only
an unresolved provenance check: a direct image needs its immutable digest, and
a source build needs both its resolved revision and built image digest. The
blocked snapshot, report, failure event, and system log remain available for
review; terminal deployments are never re-evaluated after a policy change.

An unavailable SBOM or vulnerability scan remains a warning with a remediation
action, never a pass or a blocking condition. Ignitify has no application-image
SBOM or scan attachment/verification boundary yet. Release SBOMs describe the
Ignitify control-plane artifact; attach a separate CycloneDX or SPDX SBOM for
each application image until such evidence can be verified by the control
plane.

## Incident correlation

Each accepted deployment receives an opaque correlation ID that remains stable
across idempotent retries. Deployment events expose a structured event ID and
the same correlation ID; worker logs, deployment audit activity, and related
notification delivery history carry that ID as metadata. Start incident
tracing with the correlation ID from activity or delivery history, then inspect
the bounded events and logs. The ID is safe to display, but it is not an access
token and does not include deployment secrets or provider payloads.
