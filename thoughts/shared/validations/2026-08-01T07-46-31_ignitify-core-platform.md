---
date: 2026-08-01T07:46:31Z
author: ArifPebryan
commit: 45060be
branch: main
repository: ignitify
topic: "Validation of Ignitify Core Platform"
status: needs_changes
parent: "thoughts/shared/plans/2026-07-31T23-18-20_ignitify-core-platform.md"
tags: [validation, plan, ignitify, docker, traefik, sqlite, vue, deployment]
last_updated: 2026-08-01T07:46:31Z
---

# Validation Report: Ignitify Core Platform

## Scope and Evidence

Validated current uncommitted worktree against all six plan phases. Source inspection, plan checklist review, and normal repository gates completed. Docker/Traefik opt-in integration suites were not run because they require an isolated Docker host and the plan treats them as opt-in.

## Implementation Status

- Phase 1: Workspace Authorization and Real Projects -- Partially implemented. Core project behavior exists; domain crate still depends on `serde` and `thiserror`, despite plan requirement for a dependency-free domain crate.
- Phase 2: Image Service Configuration and Encrypted Variables -- Partially implemented. Encryption and redacted DTO behavior exist; decrypted bytes are copied into ordinary `String` storage in one read path, and Compose is accepted before its planned Phase 6 sequencing point.
- Phase 3: Durable Image Deployment Worker -- Partially implemented. Durable worker/runtime path exists; restart-scan coverage and post-submit deployment selection are incomplete.
- Phase 4: Traefik Domains and Managed TLS Ingress -- Partially implemented. Domain/API/label/operator paths exist; public-suffix coverage, ACME permission enforcement, and Docker/Traefik integration coverage are incomplete.
- Phase 5: Durable Realtime Events and Log Streaming -- Partially implemented. Authenticated replay/live SSE path exists; handler coverage and bounded-retention/N+1 verification are incomplete.
- Phase 6: Hardened Docker Compose Service -- Partially implemented. Policy and fixed command path exist; fake-Docker/integration tests, stream attribution, failed-start cleanup, and mapped policy errors are incomplete.

## Automated Verification Results

- Pass: `cargo fmt --all -- --check`.
- Pass: `cargo check --workspace`.
- Pass: `cargo test --workspace` -- all workspace tests passed, including 7 API, 1 auth, 5 control-plane, 6 database, 6 domain, 2 ingress, and 6 Compose tests; Docker opt-in test passed in its normal skip/opt-in-aware path.
- Pass: `cargo clippy --workspace --all-targets -- -D warnings`.
- Pass: `cd frontend && pnpm run check` -- formatting, lint, and typecheck passed.
- Pass: `cd frontend && pnpm run build`.
- Pass: `cd frontend && pnpm run test` -- 8 files and 13 tests passed.
- No automated gate failures detected.

## Matches Plan

- Project bootstrap, authorization, duplicate-name handling, API status mapping, and frontend project states have source tests: `crates/ignitify-db/src/tests.rs:42`, `crates/ignitify-db/src/tests.rs:76`, `crates/ignitify-api/src/tests.rs:104`, `frontend/src/views/ProjectsView.spec.ts:71`.
- Image validation, encrypted persistence, secret-free DTOs, and viewer authorization exist: `crates/ignitify-domain/src/lib.rs:280`, `crates/ignitify-control-plane/src/lib.rs:1298`, `crates/ignitify-api/src/handlers/services.rs:242`, `crates/ignitify-api/src/tests.rs:475`.
- Deployment state, idempotency, snapshots, worker scan, Docker restrictions, retention, and opt-in Docker lifecycle test exist: `crates/ignitify-domain/src/lib.rs:364`, `crates/ignitify-db/src/repositories/deployments.rs:160`, `crates/ignitify-control-plane/src/lib.rs:691`, `crates/ignitify-runtime-docker/src/lib.rs:239`.
- Domain validation, generated Traefik labels, managed network, API confirmation, and operator exposure settings exist: `crates/ignitify-domain/src/lib.rs:460`, `crates/ignitify-ingress-traefik/src/lib.rs:71`, `crates/ignitify-api/src/handlers/domains.rs:113`, `infra/traefik/traefik.yaml:14`.
- SSE authentication-before-replay, subscribe-before-replay, durable lag catch-up, heartbeat, and fetch-based bearer transport exist: `crates/ignitify-api/src/handlers/streams.rs:84`, `crates/ignitify-api/src/handlers/streams.rs:101`, `crates/ignitify-api/src/handlers/streams.rs:261`, `frontend/src/lib/api/core.ts:121`.
- Compose preflight policy, staging, restricted command execution, canonicalization, and fixed `up` arguments exist: `crates/ignitify-runtime-compose/src/lib.rs:77`, `crates/ignitify-runtime-compose/src/lib.rs:285`, `crates/ignitify-runtime-compose/src/lib.rs:171`, `crates/ignitify-runtime-compose/src/lib.rs:758`.

## Deviations and Gaps

- Phase 1 dependency-free domain requirement diverges: `crates/ignitify-domain/Cargo.toml:7` retains `serde` and `thiserror`.
- Phase 2 sequencing diverges: Compose is accepted by domain/API/UI before planned Phase 6: `crates/ignitify-domain/src/lib.rs:438`, `crates/ignitify-api/src/handlers/services.rs:219`, `frontend/src/components/project/ServiceDialog.vue:139`.
- Phase 2 temporary plaintext copy is ordinary `String`: `crates/ignitify-control-plane/src/lib.rs:152`.
- Phase 3 fake-runtime test manually claims before reconciliation and lacks restart-scan verification: `crates/ignitify-control-plane/src/lib.rs:1379`, `crates/ignitify-control-plane/src/lib.rs:1437`.
- Phase 3 accepted deploy switches tab but does not select returned deployment: `frontend/src/views/ProjectDetailView.vue:124`.
- Phase 4 public-suffix rejection is limited to hardcoded entries, and ACME storage has no `acme.json` creation/mode `0600` enforcement: `crates/ignitify-domain/src/lib.rs:482`, `infra/traefik/compose.yaml:40`.
- Phase 4 opt-in Docker/Traefik integration remains absent; plan criterion is unchecked at `thoughts/shared/plans/2026-07-31T23-18-20_ignitify-core-platform.md:343`.
- Phase 5 SSE handler tests for ordered replay, reconnect, unauthorized `404`, lag catch-up, heartbeat, and leakage are absent; existing coverage only checks initial replay: `crates/ignitify-api/src/tests.rs:291`.
- Phase 5 invalid `Last-Event-ID` returns `400` instead of falling back to `after`: `crates/ignitify-api/src/handlers/streams.rs:334`.
- Phase 5 project detail performs one deployment request per service: `frontend/src/views/ProjectDetailView.vue:114`.
- Phase 6 Compose logs flatten output to `stdout`: `crates/ignitify-runtime-compose/src/lib.rs:410`.
- Phase 6 failed-start cleanup does not remove staging directory; cleanup runs only after successful `down`: `crates/ignitify-runtime-compose/src/lib.rs:352`.
- Phase 6 policy failures map to generic internal errors rather than field/path feedback: `crates/ignitify-runtime-compose/src/lib.rs:283`, `crates/ignitify-api/src/error.rs:75`.
- Phase 6 fake-Docker exact argv/environment/cwd test and opt-in Compose integration remain absent; plan criteria are unchecked at `thoughts/shared/plans/2026-07-31T23-18-20_ignitify-core-platform.md:481` and `:482`.

## Manual Testing Required

1. Sign in, create project, verify detail route and `production` environment; verify member visibility, viewer rename denial, and dashboard zero-state behavior.
2. Create/edit digest image service as owner/editor; verify viewer redaction and mutation denial; reload secrets; confirm no deployment starts in configuration-only flow.
3. Submit deployment, restart control plane during nonterminal state, verify SQLite recovery; verify idempotency replay, active conflict, stop, rollback immutability, and no public URL before domains.
4. On isolated Linux host, run Traefik staging artifact, verify private exposure, DNS route activation, typed domain removal, and `acme.json` mode `0600`.
5. Exercise SSE disconnect/reconnect, cursor expiry snapshot, unmount abort, ordering/deduplication, and secret-free browser payloads.
6. Run Compose safe/hostile fixtures and inspect staging cleanup, logs, selected proxy service, generated labels, and absence of generated secret files.

## Recommendations

- Add missing restart-scan, SSE handler, retention/N+1, fake-Docker, and opt-in Compose/Traefik integration tests before marking plan complete.
- Select returned deployment after accepted submit and replace per-service deployment loads with bounded project-level loading.
- Fix Compose failed-start staging cleanup and map policy errors to stable field/path API feedback.
- Enforce ACME file creation and permission `0600`; broaden public-suffix validation or document supported suffix scope.
- Resolve Phase 2/Phase 6 sequencing and either remove domain dependencies or revise plan requirement.

## Status

`needs_changes` -- normal automated gates pass, but plan criteria and several security/behavioral verification surfaces remain incomplete. Re-run validation after gaps are fixed and opt-in integrations run on isolated Linux Docker host.
