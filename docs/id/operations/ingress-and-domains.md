# Ingress dan domain

Ignitify menggunakan operator Traefik untuk mengekspos service yang mempunyai domain. Stack operator berada di `ignitify/infra/traefik/` dan dapat dimulai otomatis oleh backend kecuali `IGNITIFY_AUTO_START_INGRESS=false`.

## Prasyarat produksi

1. Host harus menjalankan Docker dan dapat membuat jaringan `ignitify-proxy`.
2. DNS A/AAAA setiap domain harus menunjuk ke host ingress.
3. Port HTTP/HTTPS untuk Traefik harus tersedia pada perimeter host.
4. `IGNITIFY_ACME_EMAIL` harus menggunakan alamat kontak nyata sebelum meminta sertifikat.
5. Dashboard Traefik tetap nonaktif; jangan mengaktifkannya tanpa autentikasi dan network policy yang sesuai.

Untuk operator yang dikelola manual, jalankan dari root `ignitify`:

```bash
docker compose --env-file infra/traefik/.env -f infra/traefik/compose.yaml up -d
```

Setelah backend dimulai, `GET /api/v1/runtime/status` harus menunjukkan `"ingress":"ready"` untuk user yang sudah terautentikasi.

## Menambahkan domain

1. Buat atau pilih service yang mempunyai `internal_port`.
2. Tambahkan hostname lengkap seperti `app.example.com` melalui dashboard atau `POST /api/v1/services/{service_id}/domains`.
3. Pastikan DNS selesai mengarah ke host sebelum mengharapkan TLS ACME.
4. Deploy ulang service bila perlu agar worker menerapkan route.
5. Pantau domain melalui daftar domain service dan event deployment.

Hostname harus ASCII lower-case, memiliki setidaknya satu titik, dan bukan wildcard, IP, `localhost`, maupun public suffix seperti `co.uk`.

## Diagnostik

| Gejala                        | Pemeriksaan                                                                      |
| ----------------------------- | -------------------------------------------------------------------------------- |
| Ingress unavailable           | Periksa Docker, stack Traefik, konfigurasi compose, dan status `ingress`.        |
| Domain tetap pending          | Periksa deployment/event, nama domain, DNS, dan port internal service.           |
| Domain failed                 | Periksa event deployment; kegagalan rekonsiliasi route menandai domain `failed`. |
| TLS tidak terbit              | Periksa DNS publik, port 80/443, dan `IGNITIFY_ACME_EMAIL`.                      |
| Service tidak ditemukan proxy | Pastikan service dikelola Ignitify dan terhubung ke `ignitify-proxy`.            |

Jangan menambahkan label Traefik manual ke kontainer Ignitify-managed. Worker memiliki ownership atas label dan dapat menimpa konfigurasi manual saat rekonsiliasi berikutnya.
