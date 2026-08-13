# Control Plane Backup

## Local Snapshot

Use the Ignitify binary to create a consistent SQLite snapshot together with the runtime secret file:

```sh
ignitify-core backup /srv/ignitify-backups/2026-08-09
```

The backup directory contains `ignitify.db` and `ignitify-secrets.json`. Both are required to recover encrypted provider credentials, deployment environment snapshots, certificates, and sessions. Store the directory with the same access control as production secrets.

## S3-Compatible Destination

Configure **Infrastructure > S3-compatible storage** with an HTTPS endpoint, region, bucket, prefix, and credentials. Ignitify supports AWS S3 regional endpoints, Cloudflare R2, MinIO, and compatible providers that implement AWS Signature Version 4 with path-style bucket requests.

Use a dedicated service credential with only `s3:PutObject` permission for the chosen `bucket/prefix/backups/*` path. Add `s3:GetObject` only to the separate recovery credential or operator role that downloads backups. Do not grant delete permission to the Ignitify upload credential.

For AWS S3, use a regional endpoint that matches the configured region, for example `https://s3.ap-southeast-1.amazonaws.com` with `ap-southeast-1`. For Cloudflare R2, use the account endpoint and `auto` region. A MinIO endpoint must be HTTPS and reachable from the host running Ignitify.

When an S3 destination is configured, the same `backup` command first creates the local snapshot and then uploads:

```text
<prefix>/backups/<timestamp>-<uuid>/ignitify.db
<prefix>/backups/<timestamp>-<uuid>/ignitify-secrets.json
<prefix>/backups/<timestamp>-<uuid>/manifest.json
```

The database and secret file are uploaded first. `manifest.json` includes size and SHA-256 for both files and is uploaded last; treat only prefixes with a manifest as complete backups. The client retries transient network and 5xx failures three times. The local backup directory remains intact if the remote upload fails.

The S3 destination itself is intentionally removed from every backup snapshot before the files are retained or uploaded. This prevents the upload credential from being recoverable from the same bucket. Re-enter the destination after a restore with a separately retained recovery credential.

Select **S3 managed (AES256)** when the provider supports the standard `x-amz-server-side-encryption: AES256` header. Select **Provider default** when the provider enforces its own encryption and does not accept that header. Credentials are encrypted at rest by Ignitify and are never returned through the API.

Configure a bucket lifecycle policy independently for object retention. Ignitify does not delete remote backups automatically in this release.

## Offline Restore

Restore is intentionally an offline operator action. Download the three files from one complete S3 prefix into a local directory, verify the manifest SHA-256 values, stop the Ignitify service, then run:

```sh
ignitify-core restore /srv/ignitify-backups/2026-08-09 --confirm-offline
```

The command validates the SQLite snapshot and secret file, stages replacements beside the live files, moves the current database, WAL sidecars, and runtime secrets into `data/restore-recovery-<timestamp>`, then installs the backup. Start the service only after the command reports success.

Do not restore through the web UI and do not run restore while another Ignitify process is using the database. Keep the recovery credential and its S3 endpoint details outside the Ignitify database, because a full host-loss recovery cannot read its previous S3 configuration first.

## Restore Drill

Perform the offline [restore drill](restore-drill.md) at least quarterly and after
material backup or release changes. It defines the 24-hour RPO and 60-minute
RTO objectives, validates a supported release artifact in an isolated location,
and records the required recovery evidence without starting a restored control
plane.
