# Ignitify Roadmap

This roadmap prioritizes operational safety and release reliability over
expanding the feature surface. Ignitify already manages real Docker, Compose,
SSH, Git, DNS, Traefik, monitoring, and backup operations, so changes to its
delivery and security posture have a higher priority than new integrations.

## Current Baseline

- The Rust workspace and frontend have focused unit and integration coverage,
  and the standard Rust quality gates currently pass.
- Frontend format, lint, type-check, and production build currently pass.
- The full frontend test suite has one non-deterministic `SettingsView` test:
  it times out in the complete suite but passes in isolation. The failing run
  also made an unmocked request to `localhost:3000`.
- Playwright commands exist, but the repository does not yet contain a
  Playwright configuration or an authored end-to-end test suite.
- The CI workflow runs frontend production dependency audit, Rust dependency
  audit, frontend checks/tests/builds, and Rust format/check/test/clippy gates.
  SBOM generation and end-to-end coverage are not yet enforced.
- The control-plane facade and production modules have been split by ownership:
  service configuration, deployment submission, streaming, encrypted snapshot
  handling, retry policy, worker scheduling, and reconciliation. The remaining
  reconciliation module is 790 LOC; its 768 LOC regression suite is isolated
  in a sibling test module. Workspace format, tests, and clippy pass after the
  refactor.

### Notices

- Rust reports a future-incompatibility warning for the build-time dependency
  `proc-macro-error2 v2.0.1`. It is introduced by `age` (through
  `i18n-embed-fl`) and `teloxide` (through `aquamarine`), and will become a
  compiler blocker only in a future Rust release. Track upstream upgrades or
  replacements before raising the minimum supported Rust version; do not patch
  the cryptography or notification dependency paths locally without a reviewed
  migration.

## Guiding Principles

1. Preserve the existing boundary between HTTP intent, worker reconciliation,
   and runtime adapters. HTTP handlers must not perform infrastructure effects.
2. Prefer measurable reliability and recovery improvements before new runtime
   providers or deployment modes.
3. Keep credentials encrypted at rest, redact them from logs and APIs, and
   preserve existing authorization, step-up, origin, and SSRF controls.
4. Make every release repeatable, auditable, and recoverable before expanding
   automated production actions.

## Phase 1: Deterministic Quality Gates

Target: first 30 days.

### Outcomes

- [ ] Make the frontend test suite hermetic: every network request is mocked or
  injected, and test execution does not depend on localhost or the internet.
- [ ] Diagnose and fix the `SettingsView` cross-suite flake without increasing the
  global test timeout.
- [ ] Add a tracked Playwright configuration and a minimal end-to-end suite using
  a safe, non-production fixture environment.
- [ ] Extend CI to run the E2E smoke suite after frontend and backend artifacts are
  ready.

### Initial E2E Coverage

- [ ] Bootstrap first operator, log in, refresh a session, and log out.
- [ ] Create a project and service, submit a deployment through a fake runtime,
   and observe deployment state and logs.
- [ ] Update ingress settings, including validation failure and a successful save.
- [ ] Verify unauthorized and non-operator users cannot access privileged routes.
- [ ] Confirm backup destination configuration never returns credentials.

### Definition Of Done

- [ ] The full frontend suite passes repeatedly in CI with no open network
  connections.
- [ ] Playwright has a tracked configuration and reliable smoke coverage.
- [ ] CI blocks a change when a smoke test, type check, unit test, or build fails.

## Phase 2: Supply-Chain And Recovery Readiness

Target: first 30 days, in parallel where capacity permits.

### Outcomes

- [x] Add automated dependency vulnerability review for Rust and Node dependencies.
- [ ] Add Dependabot or Renovate with review rules appropriate for infrastructure
  dependencies.
- [ ] Generate a software bill of materials for each release archive and retain it
  with the release assets.
- [ ] Review the current Rust future-incompatibility warning and upgrade or replace
  the affected dependency path before it becomes a compiler blocker.
- [ ] Establish a documented restore drill: encrypted backup, offline restore to an
  isolated location, database/runtime-secret validation, and cleanup.

### Definition Of Done

- [ ] Dependency and vulnerability checks are visible in pull requests and release
  builds.
- [ ] Every release archive has a checksum and SBOM.
- [ ] A restore drill has a documented RPO and RTO and succeeds on a supported
  release artifact.

## Phase 3: Operational Observability

Target: days 31-60.

### Outcomes

- [ ] Expose bounded operational metrics for deployment queue depth, deployment
  duration, retry counts, worker health, backup freshness, certificate/domain
  state, and remote-agent heartbeat age.
- [ ] Define alert thresholds and route alerts through existing notification
  channels.
- [ ] Add an operator-facing health summary that distinguishes control-plane,
  runtime, ingress, backup, and remote-host failure modes.
- [ ] Add structured event identifiers and correlation between audit activity,
  deployment events, and notification delivery records.

### Definition Of Done

- [ ] An operator can identify a stalled worker, failed retry, stale backup, or
  offline remote agent without inspecting raw logs.
- [ ] Alert conditions are tested and notification deduplication remains intact.

## Phase 4: Deployment And Remote-Runtime Reliability

Target: days 31-60.

### Outcomes

- [ ] Add regression coverage for worker restart during each deployment phase,
  cancellation during build and runtime startup, retry exhaustion, and ingress
  synchronization failures.
- [ ] Publish concise operator runbooks for failed deployments, rollback,
  certificate failures, remote-server connection failure, and backup recovery.
- [ ] Audit SSH and mTLS credential rotation, temporary file cleanup, known-host
  lifecycle, command timeout behavior, and failure redaction.
- [ ] Notify operators when remote agents are offline or remote authentication
  starts failing repeatedly.

### Definition Of Done

- [ ] Critical deployment state transitions are restart-safe and tested.
- [ ] Remote credentials remain encrypted, scoped, temporary on disk, and absent
  from diagnostics.
- [ ] Operators have documented recovery procedures for each critical failure mode.

## Phase 5: Secure Delivery And Production Governance

Target: days 61-90.

### Outcomes

- [ ] Add optional image provenance, SBOM, and vulnerability-policy results to
  deployment records. Start in warning mode and provide a clear remediation
  path before enforcing blocks.
- [ ] Add explicit production promotion and approval workflow while retaining an
  immutable source commit and image digest in deployment history.
- [ ] Add webhook delivery history and clear retry diagnostics without exposing
  credentials or payloads.
- [ ] Retain and visualize monitoring history using a bounded retention policy;
  define uptime and error-budget alerting.

### Definition Of Done

- [ ] Production deployments have an auditable source/image identity and an
  approval trail.
- [ ] Supply-chain signals are visible before deployment and can be governed by
  policy.
- [ ] Historical monitoring provides enough context to investigate regressions and
  alert fatigue.

## Deferred Until The Foundations Are Complete

Do not prioritize new runtime providers, canary deployment, multi-node
control-plane operation, automatic AI actions, or broad autonomous deployment
behavior before the preceding phases are delivered. These increase the blast
radius of existing infrastructure operations and require reliable testing,
observability, recovery, and governance first.

## Review Cadence

Review this roadmap every two weeks. Reorder work only using evidence from
production incidents, test reliability, vulnerability reports, restore drills,
and operator feedback. Any roadmap item that expands Docker, Compose, SSH,
DNS, ingress, or credential behavior must receive a security review and focused
regression coverage before implementation.
