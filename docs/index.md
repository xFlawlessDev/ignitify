# Ignitify

Ignitify is a self-hosted control plane for running applications, Compose services, and OCI images on your own infrastructure. This documentation describes the current implementation, not the roadmap.

## Start here

- [Getting started locally](/guide/getting-started) to set up the backend and dashboard.
- [Architecture](/concepts/architecture) to understand each crate boundary and the data flow.
- [Deployment lifecycle](/concepts/deployment-lifecycle) to understand services, environments, domains, and workers.
- [Baseline footprint benchmark](/operations/benchmark-baseline) for a controlled idle comparison with Dokploy.
- [API reference](/reference/api) for every registered HTTP route.
- [Ignitify Templates](https://github.com/xFlawlessDev/ignitify-templates) for the public template catalog and contribution workflow.

## Current capabilities

| Area       | Capabilities                                                                                                                                 |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Access     | One-time admin bootstrap, login, JWT access tokens, and cookie-based refresh tokens.                                                         |
| Workspace  | SQLite projects, owner/editor/viewer roles, services, environments, activity, and dashboard.                                                 |
| Deployment | OCI images pinned to digests, validated Compose, Git sources for applications or Compose, a queue worker, rollback, events, and log streams. |
| Operations | Runtime status, host/container metrics, Docker admin inventory and actions, and admin host/container terminals.                              |
| Ingress    | Traefik operator, service domains, ACME TLS, and the `ignitify-proxy` network.                                                               |

## Repository sections

```text
ignitify/       Rust control plane and Vue administrator dashboard
```

The administrator dashboard lives in `ignitify/frontend`.

## Operational status

The backend listens on `127.0.0.1:5656` by default; the development dashboard runs on port `6565`. Public exposure should go through a deliberately configured reverse proxy or ingress operator. Read [configuration](/operations/configuration) and [security](/reference/security) before running on a publicly reachable host.
