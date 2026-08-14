# Keamanan

Ignitify adalah control plane yang dapat mengakses Docker, source build, ingress, dan terminal. Perlakukan sebagai sistem administratif, bukan aplikasi publik biasa.

## Kontrol yang diterapkan

| Area            | Kontrol                                                                                             |
| --------------- | --------------------------------------------------------------------------------------------------- |
| Password        | Hash Argon2; password plaintext tidak dipersist.                                                    |
| Access session  | JWT berumur 15 menit; dashboard menyimpan access token di memori.                                   |
| Refresh session | Cookie HttpOnly, token yang disimpan sebagai hash, rotasi, dan deteksi reuse yang mencabut family.  |
| CSRF/origin     | Endpoint state-changing memeriksa `X-Ignitify-Request` dan origin tepercaya.                        |
| Secret config   | Environment project/service dan credential provider dienkripsi age.                                 |
| Secret output   | Nilai secret disamarkan pada read model; worker menyensor log yang dapat memuat snapshot value.     |
| Otorisasi       | Project role diverifikasi repository; tindakan Docker dan terminal memerlukan admin.                |
| Runtime image   | Image service wajib memakai digest SHA-256.                                                         |
| Compose         | Policy menolak configuration yang tidak didukung/berisiko sebelum submission.                       |
| Ingress         | Traefik hanya melihat kontainer berlabel `com.ignitify.managed=true`; dashboard operator dimatikan. |

## Checklist produksi

1. Jalankan dashboard dan API melalui HTTPS, lalu set `IGNITIFY_SECURE_COOKIES=true`.
2. Set `IGNITIFY_TRUSTED_ORIGINS` ke URL dashboard aktual, tanpa wildcard.
3. Gunakan `IGNITIFY_JWT_SECRET` dan `IGNITIFY_SECRETS_AGE_IDENTITY` yang dikelola secara aman; backup keduanya bersama database karena data terenkripsi tidak dapat dibuka tanpa identity yang sama.
4. Batasi kepemilikan dan permission `IGNITIFY_DATA_DIR`, database, checkout build, dan Compose root.
5. Jangan expose Docker socket langsung ke jaringan. Berikan least privilege pada proses/operator yang mengaksesnya.
6. Pisahkan build host bila menerima repository yang tidak sepenuhnya tepercaya, dan tetapkan resource quota Docker.
7. Arahkan DNS dan TLS melalui ingress; jangan mengekspos port API backend langsung ke internet.
8. Batasi akun admin dan audit activity deployment/provider secara rutin.

## Batas yang perlu dipahami

- Source Git menggunakan credential hanya untuk checkout sementara. Credential tidak boleh menjadi variable service, snapshot deployment, image build, atau command build.
- Executor build tidak mendukung GitHub App credential saat ini. Jangan menyimpulkan bahwa provider GitHub App otomatis dapat melakukan checkout source build.
- Build log belum menjadi stream API append-only. Jangan membangun workflow operasional yang menganggap log build tersedia dari endpoint deployment.
- Tidak ada model keamanan yang dapat menjadikan Docker aman untuk code build yang benar-benar bermusuhan tanpa isolasi host, quota, dan policy tambahan.

## Respons insiden

Jika JWT secret atau identity age diduga bocor, anggap session aktif dan data terenkripsi terpengaruh. Rotasi secret, cabut session/refresh family bila diperlukan, nilai dampak backup data, dan audit provider serta activity log. Rotasi identity age membutuhkan rencana re-enkripsi data; mengganti identity tanpa migrasi membuat nilai lama tidak dapat didekripsi.
