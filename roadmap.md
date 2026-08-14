# Ignitify Roadmap

This roadmap tracks the product's operational safety and release readiness.
The implementation foundations are complete as of `v0.2.0`; no implementation
track is currently open. New work should be justified by production evidence,
not by expanding the feature surface alone.

## Completed Foundation

### Quality And Delivery

- [x] Keep frontend tests hermetic: every network request is mocked or injected.
- [x] Keep the Settings view test suite isolated and free of cross-suite flakes.
- [x] Maintain tracked Playwright configuration and fixture-backed smoke coverage.
- [x] Run frontend checks, tests, builds, Rust format/check/test/clippy, dependency
  audits, and E2E smoke coverage in CI.
- [x] Keep release builds repeatable, checksummed, and tied to an exact Git tag.
- [x] Keep oversized production responsibilities split by ownership; the remaining
  reconciliation module is 790 LOC with its regression suite isolated separately.

### Supply Chain And Recovery

- [x] Audit Rust and Node dependencies in pull requests and release builds.
- [x] Maintain Dependabot review rules for infrastructure dependencies.
- [x] Generate and publish CycloneDX SBOMs for the Rust workspace and frontend.
- [x] Store optional deployment provenance, SBOM, and vulnerability-policy reports.
- [x] Support warning and require-provenance policy modes without false positives.
- [x] Document encrypted backup and offline restore with explicit RPO/RTO objectives.
- [x] Validate a supported release artifact through an isolated restore drill.

### Runtime Reliability And Operations

- [x] Cover worker restart, cancellation, retry exhaustion, and ingress failures.
- [x] Keep Docker, Compose, SSH, mTLS, Git, DNS, ingress, and credential handling
  behind their existing worker or adapter boundaries.
- [x] Publish runbooks for failed deployments, rollback, certificates, remote
  servers, backup recovery, and incident tracing.
- [x] Preserve encrypted credentials, temporary-file cleanup, timeout behavior,
  known-host lifecycle, and failure redaction.
- [x] Notify operators about remote-agent outages and repeated authentication errors.

### Observability And Governance

- [x] Expose bounded metrics for deployments, workers, backups, certificates,
  domains, and remote-agent heartbeats.
- [x] Route thresholded operational alerts through existing notification channels
  with delivery history, retries, and deduplication.
- [x] Provide an operator health summary for control plane, runtime, ingress,
  backup, and remote-host failure modes.
- [x] Correlate audit activity, deployment events, worker logs, alert sources,
  and notification deliveries with one opaque correlation ID.
- [x] Require explicit approval for production deployment promotion.
- [x] Retain bounded uptime history with availability and error-budget alerting.

## Evidence

### Quality

The frontend suite passed repeatedly with the unexpected-network guard enabled.
The merged CI gates for the release preparation change passed frontend quality,
Rust quality, and E2E smoke checks.

### Release And Restore

Release `v0.2.0` (`2fb8131afa0f39e8df281b930b8418b996ed2ffb`) was published from
the exact tag commit on Linux AMD64. The release contains the archive,
`SHA256SUMS`, release metadata, and 17 SBOM assets. The 2026-08-15 WSL2 drill
validated every checksum, restored a synthetic offline backup, returned SQLite
integrity `ok`, preserved the prior target files, and restored the secret file
with mode `0600`. The measured synthetic RPO was 1 second and restore plus
validation took 15 ms. The control plane was not started and no production
runtime or data was used.

### Persistence And Policy

Deployment correlation, supply-chain policy, production approvals, and uptime
history are covered by sequential migrations and regression tests. The worker
re-evaluates deployment policy after source resolution and before runtime
execution; pending production approvals cannot be claimed by the worker.

## Open Follow-Up

- [ ] Re-evaluate the Rust future-incompatibility warning for
  `proc-macro-error2 v2.0.1` when `age`/`i18n-embed-fl` or `teloxide`/`aquamarine`
  publish a compatible upgrade. Do not patch these security-sensitive paths
  locally without a reviewed migration.

## Deferred

Do not prioritize new runtime providers, canary deployment, multi-node
control-plane operation, automatic AI actions, or broad autonomous deployment
behavior until new production evidence justifies their risk and operating cost.

## Operating Rules

1. Preserve the boundary between HTTP intent, worker reconciliation, and runtime
   adapters; handlers must not perform infrastructure effects.
2. Keep credentials encrypted at rest, redacted from logs and APIs, and protected
   by authorization, step-up, origin, and SSRF controls.
3. Make releases repeatable, auditable, checksummed, SBOM-backed, and recoverable.
4. Add focused regression coverage and a security review for changes affecting
   Docker, Compose, SSH, DNS, ingress, credentials, or deployment state.

## Review Cadence

Review this document every two weeks. Reorder work only using evidence from
production incidents, test reliability, vulnerability reports, restore drills,
and operator feedback.
