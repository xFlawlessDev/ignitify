# Traefik Operator Stack

Ignitify automatically starts this operator stack on backend startup by default. The deployment worker creates only platform-owned labels and joins domain-backed services to the `ignitify-proxy` network. Set `IGNITIFY_AUTO_START_INGRESS=false` when an external operator owns the stack. Configure the control-plane hostname and application domain suffix in Infrastructure before enabling public routes. A configured control-plane hostname produces a managed HTTPS route to the loopback-only Ignitify backend through Traefik's host gateway. `IGNITIFY_ALLOWED_DOMAIN_SUFFIXES` optionally adds an operator-owned restriction.

1. For a manual or external install, start the operator stack from the repository root: `docker compose --env-file infra/traefik/.env -f infra/traefik/compose.yaml up -d`. The shared `ignitify-proxy` network is created automatically.
2. Set the ACME contact email in Ignitify's Infrastructure settings. `IGNITIFY_ACME_EMAIL` remains a bootstrap fallback for a manually started stack; the deployment worker reapplies the persisted value when it changes.
3. Point each domain's DNS A/AAAA records at this host, then verify `/health` reports `"ingress":"ready"` after restarting Ignitify.
4. Keep `IGNITIFY_TRAEFIK_DYNAMIC_DIR` consistent between the backend environment and Compose. Ignitify writes the selected custom certificate and Traefik file-provider configuration there with restrictive permissions; its generated contents are intentionally ignored by Git.

Requests arriving on port 80 or 443 without a matching application route are served by the internal `ingress-fallback` service with an Ignitify-branded HTTP 404 page. Application routers have a higher priority than this catch-all route, so registered domains remain unaffected. A browser can display the fallback over HTTPS only after it accepts a certificate for the requested hostname; Cloudflare Tunnel traffic forwarded to port 80 receives the page directly.

The page heading and message are configured in **Infrastructure > Ingress fallback**. Ignitify renders them as escaped plain text into `IGNITIFY_TRAEFIK_FALLBACK_PAGE_FILE`; Caddy reads that file on each request, so a saved change does not require a proxy restart. The release installer sets this to `/var/lib/ignitify/traefik/fallback/404.html`, which is writable by the `ignitify` service account and mounted read-only into Caddy. For a manual installation, set the same variable to a service-writable file and keep the Compose mount aligned with it.

The Traefik dashboard is disabled. The service sees Docker only through the read-only socket proxy and discovers containers only when `traefik.enable=true`; Ignitify's deployment runtime also applies `com.ignitify.managed=true` to its own containers.
