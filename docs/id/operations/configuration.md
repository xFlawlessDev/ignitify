# Konfigurasi

Simpan override backend di `ignitify/.env`; mulai dari `.env.example`. File berisi secret dan tidak boleh masuk Git.

## Backend dasar

| Variabel                        | Default                                       | Kegunaan                                               |
| ------------------------------- | --------------------------------------------- | ------------------------------------------------------ |
| `IGNITIFY_DATABASE_URL`         | `sqlite:data/ignitify.db`                     | Lokasi database SQLite.                                |
| `IGNITIFY_DATA_DIR`             | `data`                                        | Direktori secret runtime dan data lokal.               |
| `IGNITIFY_JWT_SECRET`           | dibuat saat start pertama                     | Secret JWT minimal 32 karakter untuk nilai eksplisit.  |
| `IGNITIFY_SECRETS_AGE_IDENTITY` | dibuat saat start pertama                     | Identity age untuk enkripsi configuration/environment. |
| `IGNITIFY_SECURE_COOKIES`       | `false`                                       | Set `true` bila dashboard diakses lewat HTTPS.         |
| `IGNITIFY_TRUSTED_ORIGINS`      | `http://localhost:6565,http://127.0.0.1:6565` | Daftar origin dashboard yang dipisahkan koma.          |

Backend mengikat listener ke `127.0.0.1:5656` pada implementasi saat ini. Gunakan reverse proxy lokal atau ubah implementasi secara sengaja bila perlu menerima koneksi dari interface lain. Jangan sekadar membuka firewall lalu menganggap layanan tersedia dari jaringan lain.

## Docker dan Compose

| Variabel                        | Kegunaan                                                   |
| ------------------------------- | ---------------------------------------------------------- |
| `IGNITIFY_DOCKER_HOST`          | Override endpoint Docker, misalnya `tcp://127.0.0.1:2375`. |
| `IGNITIFY_DOCKER_BIN`           | Path executable Docker bila tidak tersedia pada `PATH`.    |
| `IGNITIFY_COMPOSE_ROOT`         | Root untuk material Compose yang dikelola runtime.         |
| `IGNITIFY_AUTO_START_INGRESS`   | Set `false` bila Traefik dikelola operator eksternal.      |
| `IGNITIFY_TRAEFIK_COMPOSE_FILE` | Path Compose operator Traefik.                             |
| `IGNITIFY_ACME_EMAIL`           | Kontak ACME untuk sertifikat produksi.                     |

Jika memakai socket Docker, batasi akses proses Ignitify. Docker memberikan kemampuan tinggi pada host; dashboard dan API control plane harus diperlakukan sebagai permukaan administratif.

## Source build Git

| Variabel                           | Kegunaan                                             |
| ---------------------------------- | ---------------------------------------------------- |
| `IGNITIFY_SOURCE_BUILD_ROOT`       | Direktori sementara checkout source.                 |
| `IGNITIFY_GIT_BIN`                 | Path Git.                                            |
| `IGNITIFY_RAILPACK_BIN`            | Path binary Railpack.                                |
| `IGNITIFY_STATIC_BUILD_IMAGE`      | Builder Node yang dipin digest untuk static build.   |
| `IGNITIFY_STATIC_RUNTIME_IMAGE`    | Runtime Caddy yang dipin digest untuk static output. |
| `IGNITIFY_RAILPACK_FRONTEND_IMAGE` | Frontend Railpack yang dipin digest.                 |

Nilai default builder/runtime sudah dibundel. Override hanya saat mengganti release yang telah direview. Host build harus menyediakan Git dan Docker Buildx; produksi sebaiknya memakai build host khusus dan quota Docker.
