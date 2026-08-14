# Configuration

Store backend overrides in `ignitify/.env`, starting from `.env.example`. The file contains secrets and must not be committed to Git.

## Basic backend

| Variable                        | Default                                       | Purpose                                                     |
| ------------------------------- | --------------------------------------------- | ----------------------------------------------------------- |
| `IGNITIFY_DATABASE_URL`         | `sqlite:data/ignitify.db`                     | SQLite database location.                                   |
| `IGNITIFY_DATA_DIR`             | `data`                                        | Runtime secrets and local data directory.                   |
| `IGNITIFY_JWT_SECRET`           | generated on first start                      | JWT secret with at least 32 characters when explicitly set. |
| `IGNITIFY_SECRETS_AGE_IDENTITY` | generated on first start                      | Age identity for encrypting configuration/environment.      |
| `IGNITIFY_SECURE_COOKIES`       | `false`                                       | Set `true` when the dashboard is accessed over HTTPS.       |
| `IGNITIFY_TRUSTED_ORIGINS`      | `http://localhost:6565,http://127.0.0.1:6565` | Comma-separated dashboard origins.                          |

The backend binds to `127.0.0.1:5656` in the current implementation. Use a local reverse proxy or deliberately change the implementation when connections from another interface are required. Do not simply open the firewall and assume the service is available from another network.

## Docker and Compose

| Variable                        | Purpose                                                           |
| ------------------------------- | ----------------------------------------------------------------- |
| `IGNITIFY_DOCKER_HOST`          | Override the Docker endpoint, for example `tcp://127.0.0.1:2375`. |
| `IGNITIFY_DOCKER_BIN`           | Docker executable path when it is not on `PATH`.                  |
| `IGNITIFY_COMPOSE_ROOT`         | Root for Compose material managed by the runtime.                 |
| `IGNITIFY_AUTO_START_INGRESS`   | Set `false` when Traefik is managed by an external operator.      |
| `IGNITIFY_TRAEFIK_COMPOSE_FILE` | Traefik operator Compose path.                                    |
| `IGNITIFY_ACME_EMAIL`           | ACME contact for production certificates.                         |

When using a Docker socket, restrict access by the Ignitify process. Docker grants extensive host capabilities; treat the dashboard and control plane API as administrative surfaces.

## Git source builds

| Variable                           | Purpose                                        |
| ---------------------------------- | ---------------------------------------------- |
| `IGNITIFY_SOURCE_BUILD_ROOT`       | Temporary source checkout directory.           |
| `IGNITIFY_GIT_BIN`                 | Git path.                                      |
| `IGNITIFY_RAILPACK_BIN`            | Railpack binary path.                          |
| `IGNITIFY_STATIC_BUILD_IMAGE`      | Digest-pinned Node builder for static builds.  |
| `IGNITIFY_STATIC_RUNTIME_IMAGE`    | Digest-pinned Caddy runtime for static output. |
| `IGNITIFY_RAILPACK_FRONTEND_IMAGE` | Railpack frontend image pinned to a digest.    |

Builder/runtime defaults are bundled. Override them only when replacing a reviewed release. The build host must provide Git and Docker Buildx; production should use a dedicated build host and Docker quotas.
