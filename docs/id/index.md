# Ignitify

Ignitify adalah control plane self-hosted untuk menjalankan aplikasi, layanan Compose, dan image OCI pada infrastruktur sendiri. Dokumentasi ini menjelaskan implementasi yang ada saat ini, bukan roadmap.

## Mulai dari sini

- [Memulai secara lokal](/id/guide/getting-started) untuk menyiapkan backend dan dashboard.
- [Arsitektur](/id/concepts/architecture) untuk memahami batas setiap crate dan aliran data.
- [Siklus deployment](/id/concepts/deployment-lifecycle) untuk memahami service, environment, domain, dan worker.
- [Referensi API](/id/reference/api) untuk seluruh route HTTP yang terdaftar.
- [Ignitify Templates](https://github.com/xFlawlessDev/ignitify-templates) untuk daftar template dan kontribusi template publik.

## Kapabilitas saat ini

| Area        | Kemampuan                                                                                                                                   |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Akses       | Bootstrap admin sekali, login, access token JWT, dan refresh token berbasis cookie.                                                         |
| Workspace   | Project SQLite, peran owner/editor/viewer, service, environment, activity, dan dashboard.                                                   |
| Deployment  | Image OCI yang dipin ke digest, Compose tervalidasi, source Git untuk aplikasi atau Compose, queue worker, rollback, event, dan log stream. |
| Operasional | Status runtime, metric host/container, inventaris dan tindakan Docker admin, serta terminal host/container admin.                           |
| Ingress     | Traefik operator, domain service, TLS ACME, dan jaringan `ignitify-proxy`.                                                                  |

## Bagian repository

```text
ignitify/       Control plane Rust dan dashboard administrator Vue
```

Dashboard administrator berada di `ignitify/frontend`.

## Status operasional

Backend secara default hanya mendengarkan `127.0.0.1:5656`; dashboard pengembangan berada di port `6565`. Eksposur publik harus melalui reverse proxy atau operator ingress yang dikonfigurasi dengan sengaja. Lihat [konfigurasi](/id/operations/configuration) dan [keamanan](/id/reference/security) sebelum menjalankan pada host yang dapat diakses publik.
