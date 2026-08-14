# Siklus deployment

## Model resource

Sebuah project memiliki default environment dan banyak service. Service memiliki konfigurasi runtime, variabel, optional source configuration, dan desired generation/state. Deployment adalah snapshot immutable dari service dan environment pada saat request diterima.

```text
Project environment
       +
Service variables
       |
       v
encrypted deployment snapshot -> approval -> queue -> worker -> runtime + ingress
```

Nilai environment project digabung terlebih dahulu, lalu variabel service dengan key yang sama menimpa nilainya. Snapshot deployment dienkripsi sebelum disimpan. Perubahan environment berikutnya tidak mengubah deployment yang telah disubmit.

## Jenis service

| Jenis     | Konfigurasi                                                                                         |
| --------- | --------------------------------------------------------------------------------------------------- |
| `image`   | Reference OCI yang wajib dipin ke digest SHA-256, optional internal port dan healthcheck exec-form. |
| `compose` | YAML Compose, nama service yang diekspos, dan optional internal port.                               |

Nama service dan exposed Compose service harus berupa DNS label lower-case. YAML Compose dibatasi 1 MiB dan divalidasi oleh runtime sebelum dijalankan. Image tag seperti `nginx:latest` ditolak; gunakan `image@sha256:<64-hex>`.

## Source configuration

`source_config` memisahkan asal source dari spesifikasi runtime.

| `source`      | Kebutuhan                                                                       |
| ------------- | ------------------------------------------------------------------------------- |
| `template`    | `template` wajib diisi.                                                         |
| `compose`     | Dapat memakai YAML lokal atau repository/provider dengan repository dan branch. |
| `application` | `provider_id`, `repository`, `branch`, dan `builder` wajib diisi.               |

Builder aplikasi yang tersedia adalah `dockerfile`, `static`, `spa`, dan `railpack`. Git executor menyelesaikan branch ke commit tertentu dan menyimpan revisi tersebut pada deployment. Rollback memakai revisi tersimpan, bukan tip branch saat ini.

## State deployment

```text
queued -> preparing -> running -> healthy
                  \-> failed
running/healthy -> stopping -> stopped
healthy -> superseded
```

Worker hanya mengizinkan transisi yang sah. Permintaan deploy memakai idempotency key terlihat-ASCII dengan panjang 1 sampai 128 byte, sehingga client dapat mengulang submit tanpa membuat deployment ganda.

## Promosi dan persetujuan produksi

Default environment project adalah `production`. Request deployment atau rollback
produksi membuat snapshot immutable dengan status approval `pending`; worker
tidak dapat mengklaimnya. Pemilik project atau operator platform harus memanggil
`POST /api/v1/deployments/{deployment_id}/approve` sebelum snapshot diantrikan
untuk dieksekusi. Editor dapat meminta deployment, tetapi tidak dapat
menyetujuinya. Seorang owner dapat menyetujui request sendiri agar instalasi
single-maintainer tetap operasional, namun request dan approval tetap menjadi
aksi audit terpisah.

Riwayat menyimpan source revision dan image digest yang terkait dengan snapshot.
Image langsung sudah membawa digest immutable yang wajib. Git build mencatat
commit ter-resolve serta digest image lokal sebelum runtime mulai; rollback
menggunakan revision snapshot, bukan branch tip. Respons API menampilkan nilai
ini di `source_identity` ketika sudah diketahui. Approval yang masih pending
dapat dibatalkan dan tidak pernah memicu pekerjaan Docker, Compose, SSH, Git
build, atau ingress.

## Domain dan ingress

Domain harus berupa hostname ASCII lower-case lengkap, bukan IP, `localhost`, wildcard, atau public suffix. Domain mulai dalam status `pending`, menjadi `active` ketika route berhasil diterapkan, dan menjadi `failed` ketika rekonsiliasi route gagal.

Traefik hanya menemukan kontainer dengan label `com.ignitify.managed=true`. Service yang memiliki domain bergabung ke jaringan `ignitify-proxy`; route dan labelnya dikelola worker.

## Event, log, stop, dan rollback

- `POST /api/v1/services/{service_id}/deployments` meminta deployment produksi.
- `POST /api/v1/deployments/{deployment_id}/approve` mencatat persetujuan produksi dan mengantrekannya.
- `GET /api/v1/deployments/{deployment_id}/events` dan `/logs` menyajikan SSE yang dapat di-resume.
- `POST /api/v1/services/{service_id}/stop` meminta lifecycle berhenti.
- `POST /api/v1/deployments/{deployment_id}/rollback` mengantrekan deployment dari snapshot/revisi deployment sebelumnya.

Log yang berpotensi mengandung nilai snapshot akan disensor oleh worker. Jangan gunakan output build atau aplikasi sebagai kanal untuk mengungkap secret.

## Kebijakan rantai pasok

Setiap deployment baru menyimpan laporan rantai pasok sebelum worker berjalan.
Laporan ini mencatat pemeriksaan provenance, SBOM, dan kebijakan kerentanan.
Deployment image langsung hanya lulus provenance ketika image digest immutable
tersedia; source build mengganti laporan awal setelah source revision dan built
image digest sudah diketahui.

Operator platform mengatur kebijakan delivery menjadi `warning` (default) atau
`require-provenance`. Worker mengevaluasi kebijakan saat ini setelah source
resolution dan sebelum `runtime.start`. `require-provenance` hanya memblokir
pemeriksaan provenance yang belum terselesaikan: image langsung membutuhkan
digest immutable, sedangkan source build membutuhkan resolved revision dan
built image digest. Snapshot yang diblokir, laporan, event gagal, dan log
sistem tetap tersedia untuk ditinjau; deployment terminal tidak dievaluasi
ulang setelah kebijakan berubah.

SBOM atau vulnerability scan yang belum tersedia tetap berupa peringatan dengan
tindakan remediation, bukan pass atau kondisi pemblokiran. Ignitify belum
memiliki boundary attachment/verifikasi SBOM atau scan application-image.
SBOM release menjelaskan artefak control plane Ignitify; lampirkan SBOM
CycloneDX atau SPDX terpisah untuk setiap application image sampai bukti itu
dapat diverifikasi oleh control plane.

## Korelasi insiden

Setiap deployment yang diterima mendapat ID korelasi opaque yang tetap sama
untuk retry idempotent. Event deployment menyediakan event ID terstruktur dan
ID korelasi yang sama; log worker, activity audit deployment, dan riwayat
delivery notifikasi terkait membawa ID ini sebagai metadata. Mulai tracing
insiden dari ID korelasi pada activity atau delivery history, lalu periksa event
dan log yang dibatasi. ID ini aman ditampilkan, tetapi bukan access token dan
tidak memuat secret deployment atau payload provider.
