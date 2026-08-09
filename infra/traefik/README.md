# Traefik Operator Stack

Ignitify automatically starts this operator stack on backend startup by default. The deployment worker creates only platform-owned labels and joins domain-backed services to the `ignitify-proxy` network. Set `IGNITIFY_AUTO_START_INGRESS=false` when an external operator owns the stack. Configure the application domain suffix in Infrastructure before enabling public routes. `IGNITIFY_ALLOWED_DOMAIN_SUFFIXES` optionally adds an operator-owned restriction.

1. For a manual or external install, start the operator stack from the repository root: `docker compose --env-file infra/traefik/.env -f infra/traefik/compose.yaml up -d`. The shared `ignitify-proxy` network is created automatically.
2. Set the ACME contact email in Ignitify's Infrastructure settings. `IGNITIFY_ACME_EMAIL` remains a bootstrap fallback for a manually started stack; the deployment worker reapplies the persisted value when it changes.
3. Point each domain's DNS A/AAAA records at this host, then verify `/health` reports `"ingress":"ready"` after restarting Ignitify.
4. Keep `IGNITIFY_TRAEFIK_DYNAMIC_DIR` consistent between the backend environment and Compose. Ignitify writes the selected custom certificate and Traefik file-provider configuration there with restrictive permissions; its generated contents are intentionally ignored by Git.

The Traefik dashboard is disabled. The service sees Docker only through the read-only socket proxy and discovers containers only when `traefik.enable=true`; Ignitify's deployment runtime also applies `com.ignitify.managed=true` to its own containers.
