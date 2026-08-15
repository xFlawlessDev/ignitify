# Baseline Footprint Benchmark

This page records a reproducible idle-footprint baseline for Ignitify and
Dokploy. It is evidence from one controlled test, not a universal performance
claim.

## Scope

- Date: 2026-08-15.
- Hosts: separate, fresh Ubuntu 24.04 virtual machines with one vCPU.
- Platforms: Ignitify `v0.2.0` and Dokploy `v0.30.0`.
- State: installation complete, local health endpoint responding, and no
  user-created projects, applications, or deployments.
- Sampling: 12 samples at five-second intervals (60 seconds total), taken in
  parallel after both control planes were healthy.
- Access: SSH host keys were verified before every measurement. No credentials
  or application data were collected in this record.

The initial host capacities were not identical: Ignitify used a 1 GiB host and
Dokploy used a 2 GiB host. That historical installation snapshot is retained
below, but the normalised 1 vCPU / 2 GiB result is the primary comparison.

## Normalised Idle Baseline

The following primary baseline was collected after both hosts had been
normalised to one vCPU and 1,967 MiB of RAM. Both platforms were returned to a
fresh, no-workload state first. Twelve five-second samples were collected in
parallel after the control planes had settled. Host CPU uses deltas from
`/proc/stat`; container memory is the sum of all running product containers.

| Metric | Ignitify | Dokploy |
| --- | ---: | ---: |
| Host memory | 1,967 MiB | 1,967 MiB |
| Average host CPU | 2.17% | 3.57% |
| Average available memory | 1,539 MiB | 769 MiB |
| Running platform containers | 3 | 3 |
| Docker images | 3 | 4 |
| Docker image storage | 350.8 MB | 3.911 GB |
| Average platform-container memory | 168.3 MiB | 816.4 MiB |
| Average dedicated control-plane process | 11.5 MiB | N/A (containerised) |
| Measured control-plane total | approximately 179.8 MiB | approximately 816.4 MiB |
| Local health endpoint | HTTP 200 | HTTP 200 |

The Ignitify container total comprises Traefik (about 108.9 MiB), the Docker
read proxy (about 10.2 MiB), and the ingress fallback (about 49.2 MiB). The
Dokploy total comprises the Dokploy application (about 737.2 MiB), PostgreSQL
(about 62.7 MiB), and Dokploy Traefik (about 17.1 MiB). These are observed
values on this host pair, not capacity guarantees.

## Historical Installation Baseline

| Metric | Ignitify | Dokploy |
| --- | ---: | ---: |
| Host memory | 961 MiB | 1,967 MiB |
| Average host CPU | 1.82% | 2.04% |
| Health endpoint | HTTP 200 | HTTP 307 redirect |
| Running containers | 3 | 2 |
| Docker images | 3 | 2 |
| Docker image storage | 350.8 MB | 3.677 GB |
| Dedicated control-plane process | 56.2 MiB | N/A (containerised) |
| Platform container memory | 26.3 MiB average | 995.8 MiB cgroup average |
| Stable application and database memory | N/A | 830.1 MiB + 61.3 MiB |

Ignitify's measured control-plane process plus its platform containers was
approximately 82.5 MiB. Dokploy's repeated warm-up samples were stable at
approximately 891.4 MiB for its application and PostgreSQL containers. The
cgroup average can include memory-accounting effects such as page cache, so the
per-container measurements are retained alongside it.

The 98 MiB Ignitify host-path figure and 1 MiB Dokploy host-path figure are
intentionally excluded from the table: the two products keep different amounts
of data in Docker-managed storage, so those paths are not comparable.

## Normalised Workload Evidence

The primary workload run used the same raw Compose specification on each
normalised host: one `nginx:1.27.5-alpine` image pinned to
`sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10`.
There were no public ports or domains, external repositories, secrets, or
application data. Deployment was submitted through each product's API, and
readiness required an HTTP request from inside the running Nginx container to
`127.0.0.1:80`.

Each platform received three cold and three warm samples. Before every cold
sample, the test harness removed only the labelled/identified benchmark Nginx
container and its cached Nginx image; warm samples retained that image cache.
This controls image-pull state without bypassing either platform's deployment
workflow. Ignitify's default production environment required an explicit
approval immediately after submission. Dokploy's raw-Compose workflow has no
corresponding approval action.

| Evidence | Ignitify | Dokploy |
| --- | ---: | ---: |
| Setup API responses | bootstrap 200, project 201, service 201 | sign-up 200, project 200, environment 200, Compose 200 |
| Approval | required; all approval responses 202 | not part of this Compose run |
| Cold submit-to-healthy samples | 10,477 / 10,402 / 37,399 ms | 7,606 / 6,464 / 7,542 ms |
| Cold median | 10,477 ms | 7,542 ms |
| Cold mean | 19,426 ms | 7,204 ms |
| Warm submit-to-healthy samples | 2,185 / 2,215 / 2,205 ms | 1,286 / 337 / 299 ms |
| Warm median | 2,205 ms | 337 ms |
| Warm mean | 2,202 ms | 641 ms |
| Ignitify approval-to-healthy | cold: 10,447 / 10,352 / 37,369 ms; warm: 2,150 / 2,184 / 2,145 ms | N/A |
| In-container HTTP check | pass for all six samples | pass for all six samples |
| Nginx container memory | 2.355-2.414 MiB | 2.367-2.398 MiB |
| Docker image storage with workload | 425.2 MB | 3.911 GB |
| Control-plane restart to healthy API | 1,137 ms; workload HTTP still passed | 43,231 ms; workload HTTP still passed |

The 37,399 ms Ignitify cold sample reached `healthy` successfully but is a
material outlier. It is retained rather than discarded. The measurement counts
from submission to worker-confirmed healthy state for Ignitify and from
submission to a running, internally healthy Nginx container for Dokploy.

The benchmark harness exposed two cleanup limitations that matter to the
evidence. Ignitify's stop endpoint returned 202 after a healthy deployment but
did not remove the workload container within the 120-second observation window;
the subsequent service/project deletion returned 409. The exact labelled test
container was then removed and the test-only application state reset. Dokploy's
six timing samples completed, but the harness failed while formatting a memory
field before API cleanup; its workload remained healthy through the separately
measured control-plane restart, then the exact Nginx container and test-only
application data were removed. Neither host retained a benchmark workload.

SSH host keys were verified before every session. Output was limited to HTTP
status, terminal deployment state, elapsed time, aggregate storage, and memory.
Credentials, tokens, generated passwords, project IDs, logs, and configuration
values were kept in mode-restricted temporary files and excluded from this
record.

These are repeated measurements on equal-capacity hosts, but they are still
not a universal platform-speed ranking. They use one small image, one host pair,
serial execution, local in-container readiness, and products with different
approval and restart semantics. Public ingress latency, throughput, and failure
behaviour under load were not measured.

## Historical Workload Evidence

The following one-shot run used the same raw Compose specification on both
hosts. It contains one `nginx:1.27.5-alpine` image pinned to
`sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10`.
There were no published ports, domains, external repositories, secrets, or
application data. Readiness was an HTTP request from inside the running
container to `127.0.0.1:80`.

| Evidence | Ignitify | Dokploy |
| --- | ---: | ---: |
| Create API responses | bootstrap 200, project 201, service 201, deploy 202 | register 200, project 200, environment 200, Compose 200, deploy 200 |
| Approval | required and approved, 202 | not part of this Compose run |
| Submit to healthy | 11,509 ms | 8,936 ms |
| Approval to healthy | 11,455 ms | N/A |
| In-container HTTP check | pass | pass |
| Nginx container memory | 2.512 MiB | 2.414 MiB |
| Docker image storage after deployment | 425.2 MB | 3.911 GB |
| Product API cleanup | initial stop/delete raced and returned 409; verified residual removed during test reset | Compose delete 200, project remove 200 |

Each SSH connection verified the expected ED25519 host key before executing the
measurement. Output was limited to HTTP status, terminal deployment state,
elapsed time, Docker aggregate storage, and container memory. Credentials,
tokens, generated passwords, project IDs, logs, and configuration values were
kept in mode-restricted temporary files and excluded from this record.

These timings are evidence of one cold image deployment on unequal hosts. They
are not a valid platform-speed ranking: Ignitify also exercised its production
approval workflow, and the hosts had different memory capacities.

## Interpretation

On the normalised hosts, Ignitify used approximately 636.6 MiB less measured
idle control-plane memory and left approximately 770 MiB more host memory
available. Its installed Docker image storage was approximately 3.56 GB lower.
This is strong evidence for a smaller idle footprint on a 2 GiB VPS; it does
not measure feature equivalence, sustained application throughput, public
request latency, or reliability under load.

Dokploy completed the exact repeated submission-to-healthy workflow faster in
this run, especially with the image cache warm. That result should remain a
workflow-specific observation rather than a product-wide performance claim:
Ignitify includes a production approval transition, the readiness contracts are
not identical, and only one host pair and image were used. The much slower
Dokploy control-plane restart is likewise evidence of this restart method, not
a general availability conclusion.

Dokploy was evaluated at 2 GiB because its installation guidance specifies at
least 2 GB of memory. An earlier 1 GiB attempt was not retained as a benchmark
result: the host was resized before a stable result could be established.

## Follow-up Method

A next benchmark should first resolve and regression-test the healthy-service
stop/cleanup path exposed above. It should then repeat the same workload on
multiple fresh host pairs and record:

1. Cold and warm submission-to-healthy samples with median and spread.
2. Control-plane and workload memory during idle and controlled load.
3. HTTP latency and error rate through a real, separately assigned ingress
   hostname.
4. Full-host reboot recovery time, retained Docker storage, and product API
   cleanup success.

Keep the test isolated from production infrastructure, use synthetic data only,
and remove any temporary applications when the run is complete.
