# Offline Restore Drill

Run this drill at least quarterly and after any material change to backup,
runtime-secret, SQLite, or release packaging behavior. It verifies a supported
release artifact without connecting it to a production Docker daemon, remote
server, DNS provider, or S3 destination.

## Recovery Objectives

The service objective is an RPO of 24 hours or less. Configure scheduled
backups no less frequently than every 24 hours, retain a verified local or
downloaded copy, and include backup completion time when measuring the actual
RPO. The RTO objective is 60 minutes from selecting a complete backup to
database and runtime-secret validation on an isolated host.

Record the actual RPO and RTO from every drill. A missed backup, unavailable
recovery credential, invalid checksum, or result exceeding either objective is
an operational incident and must be tracked before the next release.

## Preparation

Use a disposable or access-restricted Linux host. Do not use a production
database path, `IGNITIFY_DATA_DIR`, Docker daemon, remote-server credential,
or ingress configuration. Obtain all of the following before stopping any
process:

- A supported `ignitify-linux-amd64.tar.gz` release archive and its
  `SHA256SUMS` from the same release.
- The matching `ignitify-rust-<crate>.cdx.json` and
  `ignitify-frontend.cdx.json` SBOMs for audit evidence.
- One complete backup directory containing `ignitify.db` and
  `ignitify-secrets.json`. For an S3 backup, download the complete prefix and
  verify its `manifest.json` before this drill.
- An empty, dedicated directory such as `/srv/ignitify-restore-drill`.

Verify the release archive before unpacking it:

```sh
cd /srv/ignitify-restore-drill/release
sha256sum --ignore-missing --check SHA256SUMS
```

The command must report the archive as `OK`. Retain the archive, checksum, and
SBOMs with the drill record; the SBOMs identify the precise release inputs.
Unpack the validated archive and locate its offline-operation binary:

```sh
tar -xzf ignitify-linux-amd64.tar.gz
export IGNITIFY_CORE="$(find . -type f -name ignitify-core -perm -u+x -print -quit)"
test -n "$IGNITIFY_CORE"
```

## Restore Procedure

Start the RTO timer after selecting the validated backup. Stop any Ignitify
process that could use the drill database. The restore command is intentionally
offline and only touches paths supplied through its environment:

```sh
export DRILL_ROOT=/srv/ignitify-restore-drill
export IGNITIFY_DATA_DIR="$DRILL_ROOT/data"
export IGNITIFY_DATABASE_URL="sqlite:$DRILL_ROOT/ignitify.db"
mkdir -p "$IGNITIFY_DATA_DIR"

"$IGNITIFY_CORE" restore "$DRILL_ROOT/backup" --confirm-offline
```

The backup directory and the target data directory must be different. The
command validates the source SQLite header and runtime-secret file, stages both
replacements, and preserves pre-existing target files under
`data/restore-recovery-<timestamp>`.

Do not start the restored control plane during this drill: restored deployment
state could otherwise reconcile real runtime infrastructure. Validate the
restored files while the instance remains offline:

```sh
sqlite3 "$DRILL_ROOT/ignitify.db" "PRAGMA integrity_check;"
test -s "$DRILL_ROOT/data/ignitify-secrets.json"
test "$(stat -c '%a' "$DRILL_ROOT/data/ignitify-secrets.json")" = 600
find "$DRILL_ROOT/data" -maxdepth 1 -type d -name 'restore-recovery-*' -print
```

`PRAGMA integrity_check` must return `ok`, the secret file must exist with mode
`0600`, and a recovery directory must be listed when a prior target existed.
Stop the RTO timer after these checks succeed.

The focused automated equivalent is:

```sh
cargo test -p ignitify-core backup_and_restore_preserve_database_and_runtime_secrets
```

It verifies snapshot creation, replacement, runtime-secret validation, and
recovery preservation in an isolated temporary directory. It does not replace
the supported-artifact drill above.

## Evidence And Cleanup

Record the date, operator, release tag and commit, backup timestamp, checksum
result, SQLite result, secret-file result, actual RPO, actual RTO, and any
follow-up issue. Redact paths that reveal customer names and never attach the
database, secret file, backup contents, or credentials to the record.

After evidence is retained, remove the dedicated drill directory using the
organization's approved secure data-retention procedure. Confirm the path is
the drill root before deleting it; never reuse this procedure against a live
Ignitify data directory.
