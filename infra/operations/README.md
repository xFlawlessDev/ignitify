# Control Plane Backup

Use the Ignitify binary to create a consistent SQLite snapshot together with the runtime secret file:

```sh
ignitify-core backup /srv/ignitify-backups/2026-08-09
```

The backup directory contains `ignitify.db` and `ignitify-secrets.json`. Both are required to recover encrypted provider credentials, deployment environment snapshots, certificates, and sessions. Store the directory with the same access control as production secrets.

Restore is intentionally an offline operator action. Stop the Ignitify service first, then run:

```sh
ignitify-core restore /srv/ignitify-backups/2026-08-09 --confirm-offline
```

The command validates the SQLite snapshot and secret file, stages replacements beside the live files, moves the current database, WAL sidecars, and runtime secrets into `data/restore-recovery-<timestamp>`, then installs the backup. Start the service only after the command reports success.

Do not restore through the web UI and do not run restore while another Ignitify process is using the database.
