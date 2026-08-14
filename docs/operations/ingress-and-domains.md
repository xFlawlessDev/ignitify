# Ingress and domains

Ignitify uses a Traefik operator to expose services with domains. The operator stack lives in `ignitify/infra/traefik/` and can be started automatically by the backend unless `IGNITIFY_AUTO_START_INGRESS=false`.

## Production prerequisites

1. The host must run Docker and be able to create the `ignitify-proxy` network.
2. Each domain's DNS A/AAAA record must point to the ingress host.
3. HTTP/HTTPS ports must be available at the host perimeter for Traefik.
4. `IGNITIFY_ACME_EMAIL` must be a real contact address before requesting certificates.
5. Keep the Traefik dashboard disabled; do not enable it without suitable authentication and network policy.

For a manually managed operator, run from the `ignitify` root:

```bash
docker compose --env-file infra/traefik/.env -f infra/traefik/compose.yaml up -d
```

After the backend starts, `GET /api/v1/runtime/status` should report `"ingress":"ready"` for an authenticated user.

## Add a domain

1. Create or select a service with an `internal_port`.
2. Add a complete hostname such as `app.example.com` through the dashboard or `POST /api/v1/services/{service_id}/domains`.
3. Ensure DNS points to the host before expecting ACME TLS.
4. Redeploy the service if needed so the worker applies the route.
5. Monitor the domain through the service domain list and deployment events.

Hostnames must be lower-case ASCII, contain at least one dot, and not be a wildcard, IP, `localhost`, or public suffix such as `co.uk`.

## Troubleshooting

| Symptom                           | Check                                                                            |
| --------------------------------- | -------------------------------------------------------------------------------- |
| Ingress unavailable               | Check Docker, the Traefik stack, Compose configuration, and `ingress` status.    |
| Domain remains pending            | Check deployment/events, the domain name, DNS, and the service's internal port.  |
| Domain failed                     | Check deployment events; route reconciliation failures mark the domain `failed`. |
| TLS is not issued                 | Check public DNS, ports 80/443, and `IGNITIFY_ACME_EMAIL`.                       |
| Service is not found by the proxy | Ensure the service is Ignitify-managed and connected to `ignitify-proxy`.        |

Do not add manual Traefik labels to Ignitify-managed containers. The worker owns those labels and may overwrite manual configuration during the next reconciliation.
