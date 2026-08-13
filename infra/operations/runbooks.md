# Operator Failure Runbooks

These procedures apply to an operator working on the Ignitify host. They are
deliberately read-only until the recovery action is explicitly named. Do not
run Docker, Compose, SSH, DNS, or restore commands against production while
investigating unless that action is part of the selected runbook.

## Common Triage

1. Open the operator health summary at `/api/v1/operations/health-summary` or
   the Dashboard operations panel.
2. Record the deployment ID, service, destination, current status, and the
   request or audit ID before taking action.
3. Review the deployment events and logs for the same deployment. Treat log
   text as diagnostic data; do not paste environment values, tokens, keys, or
   unredacted command output into an incident record.
4. Check whether the worker is progressing. A queued deployment can be safely
   retried after the underlying dependency is restored; a running deployment
   must be inspected before submitting another deployment.

## Failed Deployment

Use the service deployment/activity view to identify whether the failure is in
source build, runtime start/health, or ingress synchronization.

- Source build failure: verify the provider/repository and immutable source
  revision, then retry after the provider or builder is healthy.
- Runtime failure: inspect the bounded container/runtime logs and resource
  settings. Stop the unhealthy service before changing its configuration.
- Ingress failure: verify the domain target and certificate state, then retry
  after ingress is healthy. Do not expose the loopback control-plane port.

Submit a new deployment only after recording the failed deployment. Use cancel
for an in-progress deployment that must not continue; use stop for the running
service. The worker owns cleanup and retry state, so do not remove containers
or Compose projects manually as a first response.

## Rollback

1. Confirm the target deployment is the last known-good generation and capture
   its image digest or source revision from deployment history.
2. Submit rollback from the deployment activity view. The API operation is
   `POST /api/v1/deployments/{deployment_id}/rollback` and requires an
   idempotency key; repeat requests with the same key are safe.
3. Watch the resulting deployment events and logs until the service is healthy
   or the rollback reaches a terminal failure.
4. If rollback fails, leave the failed record intact, stop the affected service
   only when it is unsafe to keep running, and continue with the runtime or
   ingress section below.

## Certificate Or Ingress Failure

1. Check Infrastructure settings and the health summary certificate/domain
   status.
2. For Let's Encrypt, verify the ACME contact, control-plane hostname, DNS
   record, and that the validation endpoint is reachable through Traefik.
3. For a custom certificate, verify that the selected certificate has both PEM
   files and that its hostname matches the configured control-plane/domain
   names. Upload a replacement before deleting the old certificate.
4. Re-run domain verification, then retry the affected deployment. Keep the
   control plane loopback-only; never add a direct route to port `5656`.

## Remote Server Connection Failure

1. In Remote Servers, run the connection check and record only its safe
   diagnostic message and latency.
2. Verify host, port, user, firewall access, and that the installed public key
   matches the encrypted private key. If host-key verification fails, obtain a
   verified current host key out of band and replace `known_hosts`; never
   disable strict host-key checking.
3. If an agent is configured, inspect its heartbeat age. Reinstall or rotate
   the agent installation when authentication repeatedly fails, then verify a
   fresh heartbeat.
4. Retry the deployment only after the check succeeds. SSH credentials are
   decrypted only for the operation, written to mode-`0600` temporary files,
   and removed when the command or terminal ends; do not collect those files as
   diagnostics.

## Remote Credential Rotation

Rotate credentials by updating the corresponding Remote Servers or Remote
Builders record with material verified out of band. A remote-server update can
replace its private key, public key, and `known_hosts`; replace the host key
only after independently verifying the remote host identity. A remote-builder
update requires a complete CA certificate, client certificate, and client key,
plus an optional verified TLS server name.

The application encrypts both SSH and mTLS credential material at rest. During
use, SSH commands have a 10-second connection timeout and a 45-second command
timeout. Remote-builder commands have a bounded configurable timeout (15
minutes by default). The build adapter creates a mode-`0700` per-deployment
certificate directory, writes mTLS files at mode `0600`, removes them after
builder cleanup, and removes them on setup failure. Deployment logs contain
operation status rather than credential material. Do not rotate by editing the
SQLite database or by recovering temporary files.

## Backup Recovery

Use the [offline restore drill](restore-drill.md) for the complete procedure.
The short sequence is:

1. Select a complete local backup or an S3 prefix containing `manifest.json`.
2. Verify the manifest hashes and the matching release archive checksum/SBOM.
3. Stop Ignitify, set an isolated `IGNITIFY_DATA_DIR` and database URL, and run
   `ignitify-core restore <directory> --confirm-offline`.
4. Validate SQLite integrity, secret-file existence and mode `0600`, and the
   `restore-recovery-*` directory before starting any process.
5. Re-enter the S3 destination with a separately retained recovery credential;
   the destination is intentionally stripped from every backup snapshot.

Never restore through the web UI, start a restored instance against production
infrastructure, or delete the recovery directory before incident evidence is
retained.

## Incident Evidence

Record the UTC time, release/tag, deployment and service IDs, health-summary
status, selected action, outcome, and follow-up issue. Redact customer names,
repository credentials, environment values, private keys, certificate keys,
signed URLs, and raw provider error bodies.
