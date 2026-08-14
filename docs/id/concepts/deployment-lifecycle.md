# Siklus deployment

## Model resource

Sebuah project memiliki default environment dan banyak service. Service memiliki konfigurasi runtime, variabel, optional source configuration, dan desired generation/state. Deployment adalah snapshot immutable dari service dan environment pada saat request diterima.

```text
Project environment
       +
Service variables
       |
       v
encrypted deployment snapshot -> queue -> worker -> runtime + ingress
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

## Domain dan ingress

Domain harus berupa hostname ASCII lower-case lengkap, bukan IP, `localhost`, wildcard, atau public suffix. Domain mulai dalam status `pending`, menjadi `active` ketika route berhasil diterapkan, dan menjadi `failed` ketika rekonsiliasi route gagal.

Traefik hanya menemukan kontainer dengan label `com.ignitify.managed=true`. Service yang memiliki domain bergabung ke jaringan `ignitify-proxy`; route dan labelnya dikelola worker.

## Event, log, stop, dan rollback

- `POST /api/v1/services/{service_id}/deployments` mengantrekan deployment.
- `GET /api/v1/deployments/{deployment_id}/events` dan `/logs` menyajikan SSE yang dapat di-resume.
- `POST /api/v1/services/{service_id}/stop` meminta lifecycle berhenti.
- `POST /api/v1/deployments/{deployment_id}/rollback` mengantrekan deployment dari snapshot/revisi deployment sebelumnya.

Log yang berpotensi mengandung nilai snapshot akan disensor oleh worker. Jangan gunakan output build atau aplikasi sebagai kanal untuk mengungkap secret.
