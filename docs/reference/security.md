# Security

Ignitify can access Docker, source builds, ingress, and terminals. Treat it as an administrative system, not an ordinary public application.

## Applied controls

| Area                 | Control                                                                                               |
| -------------------- | ----------------------------------------------------------------------------------------------------- |
| Passwords            | Argon2 hashing; plaintext passwords are not persisted.                                                |
| Access sessions      | 15-minute JWTs; the dashboard keeps access tokens in memory.                                          |
| Refresh sessions     | HttpOnly cookie, hashed stored token, rotation, and reuse detection that revokes the family.          |
| CSRF/origin          | State-changing endpoints check `X-Ignitify-Request` and trusted origin.                               |
| Secret configuration | Project/service environments and provider credentials are encrypted with age.                         |
| Secret output        | Secret values are masked in read models; the worker redacts logs that may contain snapshot values.    |
| Authorization        | Project roles are checked by repositories; Docker and terminal actions require admin.                 |
| Runtime images       | Service images must use a SHA-256 digest.                                                             |
| Compose              | Policy rejects unsupported or risky configuration before submission.                                  |
| Ingress              | Traefik only sees containers labeled `com.ignitify.managed=true`; the operator dashboard is disabled. |

## Production checklist

1. Run the dashboard and API over HTTPS, then set `IGNITIFY_SECURE_COOKIES=true`.
2. Set `IGNITIFY_TRUSTED_ORIGINS` to the actual dashboard URL, without wildcards.
3. Use securely managed `IGNITIFY_JWT_SECRET` and `IGNITIFY_SECRETS_AGE_IDENTITY`; back up both with the database because encrypted data cannot be opened without the same identity.
4. Restrict ownership and permissions for `IGNITIFY_DATA_DIR`, the database, the build checkout, and the Compose root.
5. Never expose the Docker socket directly to the network. Give least privilege to the process/operator accessing it.
6. Isolate the build host when accepting repositories that are not fully trusted, and set Docker resource quotas.
7. Route DNS and TLS through ingress; do not expose the backend API port directly to the internet.
8. Limit administrator accounts and audit deployment/provider activity regularly.

## Important limitations

- Git source uses credentials only for temporary checkout. Credentials must not become service variables, deployment snapshots, image build inputs, or build commands.
- The build executor does not currently support GitHub App credentials. A GitHub App provider does not automatically make source checkout available.
- Build logs are not yet an append-only API stream. Do not build an operational workflow that assumes build logs are available from deployment endpoints.
- No security model can make Docker safe for truly hostile build code without host isolation, quotas, and additional policy.

## Incident response

If a JWT secret or age identity may have leaked, assume active sessions and encrypted data are affected. Rotate the secret, revoke sessions/refresh families as needed, assess backup impact, and audit providers and activity logs. Rotating the age identity requires a data re-encryption plan; replacing the identity without migration makes old values undecryptable.
