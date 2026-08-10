# Git Source Build Executor

Git-backed application and Compose services deploy a selected repository branch. The worker clones the repository into `IGNITIFY_SOURCE_BUILD_ROOT`, does not initialize submodules, records the resolved commit, and removes the checkout when the deployment input has been prepared.

The backend ships with digest-pinned defaults for the builder and runtime images. No build-related environment variables are required for the normal install. Set these only when replacing the bundled paths or image releases:

```env
IGNITIFY_SOURCE_BUILD_ROOT=/var/lib/ignitify/builds
IGNITIFY_STATIC_BUILD_IMAGE=node:22.23.1-alpine3.24@sha256:16e22a550f3863206a3f701448c45f7912c6896a62de43add43bb9c86130c3e2
IGNITIFY_STATIC_RUNTIME_IMAGE=caddy:2.11.4-alpine@sha256:98eb57d882ccd5213d1688764db10c1ca2c58a1ca3a6717a3411ad798f7a423a
IGNITIFY_RAILPACK_BIN=/usr/local/bin/railpack
IGNITIFY_RAILPACK_FRONTEND_IMAGE=ghcr.io/railwayapp/railpack-frontend:latest@sha256:bc73534934e7929ab3dc41765fb7e25c8c69d9be98c43ef8792fea51f65317bd
```

Application sources support `dockerfile`, `static`, and `railpack`. `dockerfile` uses the selected repository Dockerfile. `static` runs the configured build command inside the configured pinned builder image and serves the configured output directory on port 80 through Caddy. `railpack` follows Railpack's production flow: `railpack prepare` creates a build plan and Docker Buildx loads the result using the configured Railpack frontend image.

Compose sources load a Compose file from the selected repository; the file path defaults to `docker-compose.yml` and can be changed in the service configuration. The checked-out Compose YAML becomes the runtime input. For a new Git Compose source, Ignitify uses the first declared Compose service for managed ingress; the internal port remains an Ignitify setting.

## Compose Source Policy

Git Compose deploys reviewed prebuilt images. Every service image must use an exact SHA-256 digest, such as `registry.example/app@sha256:...`. Ignitify owns ingress and host isolation, so Compose sources must not declare `build`, host `ports`, bind mounts, privileged services, device access, host networking, raw Traefik labels, external networks or volumes, `env_file`, or other host-escape settings.

Use service or project environment variables for runtime values. Repositories that build a Docker image must use the Application source with the Dockerfile builder instead of Compose. Repositories that already publish a digest-pinned image can use a Compose source without a `build` section or host port mapping.

Once a build resolves its commit, that revision is stored with the deployment. A rollback reuses the stored revision instead of the current branch tip.

The backend host must have Git and Docker Buildx. The release package should place a compatible `railpack` binary beside the Ignitify executable; the executor automatically uses that binary, then falls back to `railpack` on `PATH`. Provider token or OAuth credentials are read only by the backend process and are placed in a temporary Git configuration file for checkout; they are not written to the service, deployment snapshot, generated image, or build command. GitHub App credentials are not yet supported by this executor.

Set resource quotas for the Docker daemon and run this worker on a dedicated build host before accepting untrusted repositories. Build logs intentionally stay out of the API until an append-only, bounded build-log stream is added.
