# Ignitify Roadmap

This roadmap prioritizes operational safety and release reliability over
expanding the feature surface. Ignitify already manages real Docker, Compose,
SSH, Git, DNS, Traefik, monitoring, and backup operations, so changes to its
delivery and security posture have a higher priority than new integrations.

## Current Baseline

- The Rust workspace and frontend have focused unit and integration coverage,
  and the standard Rust quality gates currently pass.
- Frontend format, lint, type-check, and production build currently pass.
- The frontend test setup rejects unexpected `fetch` calls, and the full suite
  passes with API calls mocked or injected. `SettingsView` now mocks its API
  boundary directly and cleans up every mounted app between cases.
- Playwright has a tracked Chromium configuration and a fixture-backed smoke
  suite. Its fake API is in-memory, records unhandled calls, and cannot invoke
  a real backend or runtime.
- The CI workflow runs frontend production dependency audit, Rust dependency
  audit, frontend checks/tests/builds, Rust format/check/test/clippy gates,
  then the Playwright smoke suite against the frontend bundle after the
  frontend and Rust gates pass. Release builds now audit dependencies, generate
  CycloneDX SBOMs for Rust and frontend dependencies, and retain them beside
  checksummed release assets.
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

- [x] Make the frontend test suite hermetic: every network request is mocked or
  injected, and test execution does not depend on localhost or the internet.
- [x] Diagnose and fix the `SettingsView` cross-suite flake without increasing the
  global test timeout.
- [x] Add a tracked Playwright configuration and a minimal end-to-end suite using
  a safe, non-production fixture environment.
- [x] Extend CI to run the E2E smoke suite after frontend and backend artifacts are
  ready.

### Initial E2E Coverage

- [x] Bootstrap first operator, log in, refresh a session, and log out.
- [x] Create a project and service, submit a deployment through a fake runtime,
   and observe deployment state and logs.
- [x] Update ingress settings, including validation failure and a successful save.
- [x] Verify unauthorized and non-operator users cannot access privileged routes.
- [x] Confirm backup destination configuration never returns credentials.

### Definition Of Done

- [x] Confirm the full frontend suite passes repeatedly in CI with no open
  network connections.
- [x] Playwright has a tracked configuration and locally validated smoke coverage.
- [x] CI blocks a change when a smoke test, type check, unit test, or build fails.

Evidence: `pnpm run test` passed three consecutive times (32 files, 89 tests
per run) with the global unexpected-network guard enabled. GitHub Actions
also passed the complete frontend, Rust, and E2E gates on both the dependency
integration PR (run `31690275096`) and the resulting `main` merge commit (run
`31691099175`), with no unhandled network request reported by the frontend
suite.

## Phase 2: Supply-Chain And Recovery Readiness

Target: first 30 days, in parallel where capacity permits.

### Outcomes

- [x] Add automated dependency vulnerability review for Rust and Node dependencies.
- [x] Add Dependabot or Renovate with review rules appropriate for infrastructure
  dependencies.
- [x] Generate a software bill of materials for each release archive and retain it
  with the release assets.
- [ ] Review the current Rust future-incompatibility warning and upgrade or replace
  the affected dependency path before it becomes a compiler blocker.
- [x] Establish a documented restore drill: encrypted backup, offline restore to an
  isolated location, database/runtime-secret validation, and cleanup.

### Definition Of Done

- [x] Dependency and vulnerability checks are visible in pull requests and release
  builds.
- [x] Every release archive has a checksum and SBOM.
- [ ] A restore drill has a documented RPO and RTO and succeeds on a supported
  release artifact.

The Rust warning remains open after review on 2026-08-13: `proc-macro-error2`
2.0.1 is the latest available release, and both `age` (via `i18n-embed-fl`) and
`teloxide` (via `aquamarine`) still require it. Re-evaluate it when either
upstream dependency updates; no local patch is justified for those
security-sensitive dependency paths.

The supported-artifact restore drill was exercised on 2026-08-13 in isolated
WSL2 Linux AMD64 using release `v0.1.3` (`7fa5d955`): the release archive
checksum was valid, a synthetic offline backup restored successfully, SQLite
integrity returned `ok`, the restored secret file had mode `0600`, and prior
target files were retained in the recovery directory. The synthetic backup age
was 99 seconds (RPO), and end-to-end restore plus validation took 1,499 ms
(RTO). The restored control plane was not started and no production data or
runtime was used.

The item remains open because `v0.1.3` was published before its release workflow
generated and attached Rust/frontend SBOM assets. Close it only after repeating
the drill with a release whose archive, `SHA256SUMS`, and SBOMs all match, then
retain the redacted evidence. This is a release-evidence gap, not an
implementation failure.

## Phase 3: Operational Observability

Target: days 31-60.

### Outcomes

- [x] Expose bounded operational metrics for deployment queue depth, deployment
  duration, retry counts, worker health, backup freshness, certificate/domain
  state, and remote-agent heartbeat age.
- [x] Define alert thresholds and route alerts through existing notification
  channels.
- [x] Add an operator-facing health summary that distinguishes control-plane,
  runtime, ingress, backup, and remote-host failure modes.
- [x] Add structured event identifiers and correlation between audit activity,
  deployment events, worker logs, operational-alert sources, and notification
  delivery records.

### Definition Of Done

- [x] An operator can identify a stalled worker, failed retry, stale backup, or
  offline remote agent without inspecting raw logs.
- [x] Alert conditions are tested and notification deduplication remains intact.
- [x] An operator can trace a deployment incident from one opaque correlation ID
  across activity, deployment events/logs, and notification delivery history.

Evidence: migration `0035_event_correlation.sql` assigns every new deployment a
UUID correlation ID, preserves it through lifecycle events and worker logs, and
backfills legacy deployment records with a deterministic ID. Notification
deliveries retain the same ID without changing their deduplication key; alerts
use structured source IDs. The API, SSE streams, activity history, and delivery
history expose only the opaque identifier. The operations runbook starts incident
triage from that identifier, and database/API regression coverage verifies the
links.

## Phase 4: Deployment And Remote-Runtime Reliability

Target: days 31-60.

### Outcomes

- [x] Add regression coverage for worker restart during each deployment phase,
  cancellation during build and runtime startup, retry exhaustion, and ingress
  synchronization failures.
- [x] Publish concise operator runbooks for failed deployments, rollback,
  certificate failures, remote-server connection failure, and backup recovery.
- [x] Audit SSH and mTLS credential rotation, temporary file cleanup, known-host
  lifecycle, command timeout behavior, and failure redaction.
- [x] Notify operators when remote agents are offline or remote authentication
  starts failing repeatedly.

### Definition Of Done

- [x] Critical deployment state transitions are restart-safe and tested.
- [x] Remote credentials remain encrypted, scoped, temporary on disk, and absent
  from diagnostics.
- [x] Operators have documented recovery procedures for each critical failure mode.

## Phase 5: Secure Delivery And Production Governance

Target: days 61-90.

### Outcomes

- [x] Add optional image provenance, SBOM, and vulnerability-policy results to
  deployment records. Start in warning mode and provide a clear remediation
  path before enforcing blocks.
- [x] Add explicit production promotion and approval workflow while retaining an
  immutable source commit and image digest in deployment history.
- [x] Add webhook delivery history and clear retry diagnostics without exposing
  credentials or payloads.
- [x] Retain and visualize monitoring history using a bounded retention policy;
  define uptime and error-budget alerting.

### Definition Of Done

- [x] Production deployments have an auditable source/image identity and an
  approval trail.
- [ ] Supply-chain signals are visible before deployment and can be governed by
  policy.
- [x] Historical monitoring provides enough context to investigate regressions and
  alert fatigue.

Evidence: migration `0036_deployment_supply_chain_reports.sql` stores an
optional per-deployment report. Immutable direct-image digests and resolved
source-build revision/image pairs pass the provenance check. Application-image
SBOM and vulnerability evidence that is not attached remains an explicit
warning with remediation; the worker does not block a deployment. The API and
service deployment panel expose the report before runtime execution advances,
and domain/database/control-plane/API/frontend regression coverage verifies
the warning-mode contract.

Evidence: migration `0037_deployment_production_approvals.sql` records pending
and approved production promotion states. Owners and platform operators must
perform an explicit approval action before the worker can claim a production
snapshot; editors can request but cannot approve. Approval events and audit
records use the deployment correlation ID. The API and deployment UI expose the
approval trail and immutable source/image identity when it is known; database,
control-plane, API, and frontend regression coverage verifies that pending work
is never executed.

Evidence: migration `0038_uptime_check_history.sql` records timestamped safe
uptime check outcomes separately from the bounded 30-check status strip. It
retains at most 30 days and 1,000 checks for each monitor, while the owner-scoped
history API caps chart responses at 500 points and calculates availability from
the complete selected window. The uptime UI exposes 24-hour, 7-day, and 30-day
views. A monitor with at least three checks sends a transition-deduplicated
operational alert after it consumes the 1% 24-hour error budget, and sends a
resolved notification when it recovers.

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
