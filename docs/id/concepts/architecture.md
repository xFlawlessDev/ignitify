# Arsitektur

Ignitify memisahkan HTTP, domain, persistence, dan external side effect. Pemisahan ini menjaga handler tetap tipis, membuat validasi dapat diuji tanpa runtime Docker, dan membatasi tempat yang memiliki akses ke rahasia atau host.

## Komponen runtime

```text
Browser dashboard
  -> Vue Router / Pinia / typed API client
  -> Vite proxy pada pengembangan
  -> Axum API
       -> AuthService + SQLite repositories
       -> ServiceControl / ControlHandle
       -> deployment worker
            -> Docker runtime atau Compose runtime
            -> Git source builder
            -> Traefik ingress
```

`ignitify-core` membaca konfigurasi, membuat secret runtime bila perlu, membuka SQLite, membuat adapter Docker/Compose/Traefik/Git, lalu memulai worker dan router Axum. Listener default adalah `127.0.0.1:5656`.

## Crate dan tanggung jawab

| Crate                      | Tanggung jawab                                                                    |
| -------------------------- | --------------------------------------------------------------------------------- |
| `ignitify-core`            | Composition root, listener, secret runtime, dan readiness dependency.             |
| `ignitify-api`             | Route, HTTP DTO, extract auth/origin, cookie, stream SSE, dan mapping error aman. |
| `ignitify-auth`            | Argon2 password, JWT access token, refresh token berotasi, dan session.           |
| `ignitify-db`              | Pool SQLite, embedded migration, model persistence, dan repository berotorisasi.  |
| `ignitify-domain`          | Identifier, service spec, status deployment/domain, dan aturan input.             |
| `ignitify-control-plane`   | Enkripsi environment, submit deployment, worker, lifecycle, event/log stream.     |
| `ignitify-runtime-docker`  | Inventaris, metric, aksi kontainer, dan runtime image OCI.                        |
| `ignitify-runtime-compose` | Validasi dan lifecycle service Compose.                                           |
| `ignitify-ingress-traefik` | Jaringan dan label Traefik untuk domain service.                                  |
| `ignitify-source-git`      | Checkout source, resolusi commit, dan builder Dockerfile/static/Railpack.         |
| `ignitify-terminal`        | Terminal PTY yang diproteksi untuk host dan kontainer.                            |

## Data yang bertahan

SQLite menyimpan user, refresh token hash, project dan membership, service, deployment, domain, activity, provider, serta konfigurasi source. Migrasi ada di `ignitify-db/migrations/` dan di-embed oleh crate database.

Nilai environment project dan service tidak disimpan sebagai plaintext. `ServiceControl` mengenkripsi nilai dengan age, menggunakan identity runtime yang disimpan terpisah dari database. Saat dibaca, nilai secret tetap dimask; nilai non-secret hanya dapat dibuka bagi peran yang dapat mengelola service.

## Otorisasi

Autentikasi menghasilkan `AuthenticatedUser` dengan peran `admin` atau `user`. Di dalam project, peran membership adalah:

| Peran    | Izin                                                         |
| -------- | ------------------------------------------------------------ |
| `owner`  | Mengubah project dan mengelola service/environment.          |
| `editor` | Mengelola service/environment, tanpa mengubah project.       |
| `viewer` | Membaca resource yang dapat diakses.                         |
| `admin`  | Akses lintas project dan operasi host yang dilindungi admin. |

Repository menerima actor dan mengevaluasi akses sebelum menghasilkan data. Handler tidak boleh mengandalkan penyaringan UI untuk menerapkan akses.

## Observability dan health

`GET /health` adalah probe dasar tanpa autentikasi. `GET /api/v1/runtime/status` memeriksa database, runtime, worker, ingress, dan metric host sebagai status `ready` atau `unavailable`. Metric sistem detail ada pada `GET /api/v1/runtime/metrics`; inventaris Docker tersedia melalui endpoint runtime.

Check monitor uptime disimpan terpisah dari konfigurasi monitor dengan batas 30
hari dan 1.000 check untuk setiap monitor. History per owner menampilkan status,
latency, dan error aman bertimestamp untuk inspeksi 24 jam, 7 hari, atau 30 hari.
Target availability 99% dalam 24 jam terakhir hanya mengirim alert operasional
yang terdeduplikasi setelah setidaknya tiga check tersedia.

Event dan log deployment menggunakan SSE. Stream dapat melakukan replay dari cursor yang disimpan (`Last-Event-ID` atau `after`) dan mengirim snapshot jika cursor client lebih tua dari retensi event.
