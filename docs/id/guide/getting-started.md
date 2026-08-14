# Memulai secara lokal

Ignitify terdiri dari control plane (`ignitify/`) dan dashboard administrator (`ignitify/frontend/`). Jalankan komponen yang diperlukan untuk pekerjaan Anda.

## Prasyarat

- Rust stable dengan edition 2024 dan Cargo.
- Node.js 22 atau lebih baru dan Corepack/pnpm.
- Docker Engine, Docker Compose v2, dan Docker Buildx untuk menjalankan deployment.
- Git untuk source build. Railpack dibutuhkan untuk builder `railpack`.

## Siapkan control plane

Dari root repository `ignitify`:

```powershell
Copy-Item .env.example .env
cargo check --workspace
cargo test --workspace
```

`IGNITIFY_JWT_SECRET` dan identitas age tidak wajib disetel untuk pengembangan: backend membuat dan menyimpan rahasia runtime pertama kali di `IGNITIFY_DATA_DIR` (default `data/`). Untuk lingkungan yang bertahan lama, set nilai eksplisit dan lindungi direktori data tersebut.

Jalankan API saat Anda memang membutuhkan runtime lokal:

```bash
cargo run -p ignitify-core
```

API mendengarkan `http://127.0.0.1:5656`; pemeriksaan tanpa autentikasi tersedia di `GET /health`.

## Siapkan dashboard administrator

Dari `ignitify/frontend`:

```bash
corepack enable
vp install
vp run check
vp run test
vp dev
```

Dashboard tersedia di `http://127.0.0.1:6565` dan mem-proxy `/api` ke port backend `5656`, termasuk WebSocket terminal. Ketika database belum memiliki pengguna, layar login meminta bootstrap administrator pertama.

## Bootstrap pertama

Bootstrap hanya dapat dilakukan sekali. Dari dashboard, buat username dan password admin. Untuk client API, request state-changing memerlukan origin tepercaya dan header `X-Ignitify-Request: 1`:

```powershell
$body = @{ username = "admin"; password = "gunakan-password-kuat" } | ConvertTo-Json
Invoke-RestMethod -Method Post `
  -Uri "http://127.0.0.1:5656/api/v1/auth/bootstrap" `
  -Headers @{ Origin = "http://127.0.0.1:6565"; "X-Ignitify-Request" = "1" } `
  -ContentType "application/json" `
  -Body $body
```

Simpan access token hasil respons hanya di memori client. Refresh token dikelola sebagai cookie HttpOnly oleh server.

## Pemeriksaan yang direkomendasikan

| Area                       | Perintah                                                |
| -------------------------- | ------------------------------------------------------- |
| Rust format                | `cargo fmt --all -- --check`                            |
| Rust lint                  | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust test                  | `cargo test --workspace`                                |
| Dashboard format/lint/type | `vp run check` dari `ignitify/frontend`                 |
| Dashboard test             | `vp run test` dari `ignitify/frontend`                  |

Lanjutkan ke [konfigurasi](/id/operations/configuration) sebelum mengaktifkan Docker, ingress, atau build dari repository Git.
