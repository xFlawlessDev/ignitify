# API control plane

Base URL local: `http://127.0.0.1:5656`. Semua route control plane berada di bawah `/api/v1`, kecuali `GET /health`.

## Konvensi

- Request dan respons memakai JSON kecuali upload multipart, SSE, dan WebSocket terminal.
- Route terproteksi membutuhkan `Authorization: Bearer <access-token>`.
- Request yang mengubah state juga membutuhkan `X-Ignitify-Request: 1` dan `Origin` yang ada dalam `IGNITIFY_TRUSTED_ORIGINS`.
- Respons error tidak mengungkap detail database, token, atau runtime. Status `400`, `401`, `403`, `404`, `409`, dan `500` mengikuti kelas kegagalan umum.
- Identifier resource adalah UUID. Gunakan idempotency key ketika membuat deployment.

## Autentikasi

| Method | Path                     | Auth   | Keterangan                                               |
| ------ | ------------------------ | ------ | -------------------------------------------------------- |
| `GET`  | `/health`                | Tidak  | Probe proses dasar.                                      |
| `GET`  | `/api/v1/auth/bootstrap` | Tidak  | Memeriksa apakah admin pertama diperlukan.               |
| `POST` | `/api/v1/auth/bootstrap` | Tidak  | Membuat admin pertama; hanya sekali.                     |
| `POST` | `/api/v1/auth/login`     | Tidak  | Membuat session dan refresh cookie.                      |
| `POST` | `/api/v1/auth/refresh`   | Cookie | Memutar refresh token dan menerbitkan access token baru. |
| `POST` | `/api/v1/auth/logout`    | Cookie | Mencabut refresh session dan menghapus cookie.           |
| `GET`  | `/api/v1/auth/me`        | Ya     | Mengembalikan user saat ini.                             |

Body bootstrap/login memakai `{ "username": "...", "password": "..." }`. Respons session berisi `access_token`, `token_type`, `expires_at`, dan `user`; cookie refresh dikirim dengan `Set-Cookie`.

## Dashboard dan provider

| Method            | Path                                           | Keterangan                                              |
| ----------------- | ---------------------------------------------- | ------------------------------------------------------- |
| `GET`             | `/api/v1/dashboard`                            | Ringkasan project, service, dan deployment untuk actor. |
| `GET`, `POST`     | `/api/v1/providers`                            | List atau buat provider source.                         |
| `POST`            | `/api/v1/providers/github/manifest`            | Memulai GitHub App manifest flow.                       |
| `GET`             | `/api/v1/providers/github/manifest/callback`   | Callback manifest GitHub.                               |
| `PATCH`, `DELETE` | `/api/v1/providers/{provider_id}`              | Ubah atau hapus provider.                               |
| `POST`            | `/api/v1/providers/{provider_id}/test`         | Uji koneksi provider.                                   |
| `GET`             | `/api/v1/providers/{provider_id}/repositories` | Daftar repository yang dapat diakses.                   |
| `GET`             | `/api/v1/providers/{provider_id}/branches`     | Daftar branch untuk repository provider.                |

Provider credential dienkripsi pada backend. GitHub App manifest flow dan provider discovery tidak berarti setiap credential provider dapat dipakai oleh executor build; executor Git saat ini tidak mendukung GitHub App credential.

## Runtime dan terminal

| Method   | Path                                                 | Akses | Keterangan                                                      |
| -------- | ---------------------------------------------------- | ----- | --------------------------------------------------------------- |
| `GET`    | `/api/v1/runtime/status`                             | User  | Readiness database/runtime/worker/ingress dan ringkasan metric. |
| `GET`    | `/api/v1/runtime/metrics`                            | User  | CPU, memori, disk, network, dan metric container.               |
| `GET`    | `/api/v1/runtime/containers`                         | User  | Inventaris kontainer bila runtime tersedia.                     |
| `GET`    | `/api/v1/runtime/containers/{container_id}/details`  | Admin | Detail konfigurasi, mount, network, dan label.                  |
| `GET`    | `/api/v1/runtime/containers/{container_id}/logs`     | Admin | Log kontainer.                                                  |
| `POST`   | `/api/v1/runtime/containers/{container_id}/upload`   | Admin | Upload multipart, maksimum 8 MiB.                               |
| `DELETE` | `/api/v1/runtime/containers/{container_id}`          | Admin | Menghapus kontainer.                                            |
| `GET`    | `/api/v1/terminal`                                   | Admin | Upgrade WebSocket terminal host.                                |
| `GET`    | `/api/v1/runtime/containers/{container_id}/terminal` | Admin | Upgrade WebSocket terminal kontainer.                           |

Upload menerima field multipart `file` dan optional `destination` (default `/tmp`). Terminal melakukan autentikasi dan validasi origin pada WebSocket; gunakan client dashboard sebagai referensi protokol.

## Project, environment, dan service

| Method                   | Path                                        | Keterangan                                         |
| ------------------------ | ------------------------------------------- | -------------------------------------------------- |
| `GET`, `POST`            | `/api/v1/projects`                          | List project yang dapat diakses atau buat project. |
| `GET`, `PATCH`           | `/api/v1/projects/{project_id}`             | Detail atau ubah project.                          |
| `GET`, `PUT`             | `/api/v1/projects/{project_id}/environment` | Baca atau ganti environment default project.       |
| `GET`                    | `/api/v1/projects/{project_id}/deployments` | Deployment seluruh service project.                |
| `GET`                    | `/api/v1/projects/{project_id}/activity`    | Activity project.                                  |
| `GET`, `POST`            | `/api/v1/projects/{project_id}/services`    | List atau buat service.                            |
| `GET`, `PATCH`, `DELETE` | `/api/v1/services/{service_id}`             | Baca, ubah, atau hapus service.                    |
| `GET`, `POST`            | `/api/v1/services/{service_id}/domains`     | List atau tambahkan domain.                        |
| `DELETE`                 | `/api/v1/domains/{domain_id}`               | Hapus domain dengan konfirmasi.                    |

Contoh request service image:

```json
{
  "name": "web",
  "kind": "image",
  "image_reference": "nginx@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "internal_port": 8080,
  "healthcheck": ["/bin/sh", "-c", "wget -qO- http://localhost:8080/health"],
  "variables": [
    { "key": "LOG_LEVEL", "value": "info", "is_secret": false },
    { "key": "DATABASE_URL", "value": "secret", "is_secret": true }
  ]
}
```

Untuk service Compose, gunakan `kind: "compose"`, `compose_yaml`, `exposed_service`, `internal_port`, dan `variables`. `source_config` dapat ditambahkan untuk template, Git Compose, atau application source. Nilai secret respons tidak dikembalikan sebagai plaintext.

Environment project memakai bentuk berikut. Untuk mempertahankan secret yang sudah ada, kirim `value: null` dengan `is_secret: true`; nilai baru diperlukan untuk variable non-secret.

```json
{
  "variables": [
    { "key": "REGION", "value": "id", "is_secret": false },
    { "key": "API_KEY", "value": null, "is_secret": true }
  ]
}
```

## Deployment dan stream

| Method        | Path                                           | Keterangan                                      |
| ------------- | ---------------------------------------------- | ----------------------------------------------- |
| `GET`, `POST` | `/api/v1/services/{service_id}/deployments`    | List deployment service atau submit deployment. |
| `POST`        | `/api/v1/services/{service_id}/stop`           | Meminta penghentian service.                    |
| `GET`         | `/api/v1/deployments/{deployment_id}`          | Detail deployment.                              |
| `POST`        | `/api/v1/deployments/{deployment_id}/rollback` | Submit rollback dari snapshot deployment.       |
| `GET`         | `/api/v1/deployments/{deployment_id}/events`   | SSE event lifecycle.                            |
| `GET`         | `/api/v1/deployments/{deployment_id}/logs`     | SSE line log.                                   |

Submit deployment membutuhkan header idempotency key sesuai kontrak client. Untuk melanjutkan SSE, kirim `Last-Event-ID` atau parameter query `after=<sequence>`. Stream mengirim event `snapshot` jika cursor lebih lama dari data yang masih tersedia, heartbeat sekitar 15 detik, dan event `log` untuk stream log.

## Sumber kontrak

Daftar route di atas berasal dari `ignitify/crates/ignitify-api/src/routes.rs`. DTO respons dan validasi spesifik berada pada `handlers/` dan `ignitify-domain`. Saat mengubah kontrak, perbarui route, typed client dashboard, test, dan halaman ini dalam satu perubahan.
