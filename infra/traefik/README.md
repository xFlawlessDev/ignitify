# Traefik Operator Stack

Ignitify automatically starts this operator stack on backend startup by default. The deployment worker creates only platform-owned labels and joins domain-backed services to the `ignitify-proxy` network. Set `IGNITIFY_AUTO_START_INGRESS=false` when an external operator owns the stack.

1. For a manual or external install, start the operator stack from the repository root: `docker compose --env-file infra/traefik/.env -f infra/traefik/compose.yaml up -d`. The shared `ignitify-proxy` network is created automatically.
2. Set `IGNITIFY_ACME_EMAIL` to a real contact address for production certificates. The bundled default keeps the stack bootable for local or staging installs.
3. Point each domain's DNS A/AAAA records at this host, then verify `/health` reports `"ingress":"ready"` after restarting Ignitify.

The Traefik dashboard is disabled. The service sees Docker only through the read-only socket proxy and discovers only containers labelled `com.ignitify.managed=true`.
