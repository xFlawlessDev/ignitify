# Pengembangan

## Peta repository

```text
ignitify/
  crates/
    ignitify-core/             Komposisi runtime dan listener
    ignitify-api/              Axum routes, handler, DTO, HTTP error
    ignitify-auth/             Password, JWT, refresh session
    ignitify-db/               SQLite, migrasi, model, repository
    ignitify-domain/           Tipe domain dan validasi
    ignitify-control-plane/    Queue, worker, snapshot, stream
    ignitify-runtime-*/        Adapter Docker dan Compose
    ignitify-ingress-traefik/  Adapter Traefik
    ignitify-source-git/       Checkout dan builder Git
    ignitify-terminal/         PTY host/container
  frontend/                    Dashboard Vue 3
  infra/                       Stack Traefik dan dokumentasi executor
```

## Batas arsitektur

Dependency backend mengalir satu arah: `core -> api -> auth -> db -> domain`. `ignitify-api` boleh memakai kontrak dari database/domain sebagai adapter, tetapi crate yang lebih rendah tidak boleh mengimpor HTTP atau runtime.

- Tambahkan endpoint pada `ignitify-api/src/routes.rs` dan handler pada `handlers/<domain>.rs`.
- Letakkan validasi bisnis dan tipe yang bebas I/O di `ignitify-domain`.
- Tambahkan perubahan data sebagai migrasi SQL bernomor di `ignitify-db/migrations/` dan akses melalui repository.
- External side effect deployment berada di `ignitify-control-plane` dan crate adapter, bukan handler HTTP.
- `ignitify-core/src/main.rs` hanya menyusun dependency, worker, dan listener.

## Workflow perubahan

1. Tulis atau perbarui validasi domain dan test unitnya.
2. Tambahkan migrasi jika kontrak persistence berubah.
3. Ubah repository dan service/control plane sebelum handler HTTP.
4. Hubungkan typed API client, composable, store, lalu view dashboard.
5. Tambahkan test focused untuk otorisasi, state, persistence, atau regresi UI.
6. Jalankan quality gate untuk area yang berubah.

## Konvensi backend

- Rust 2024, error enum bertipe per crate, dan `Result` dengan `?` untuk kegagalan yang dapat dipulihkan.
- Tidak ada `unwrap()` atau `expect()` pada path produksi.
- SQL memakai bind parameter; transaksi melingkupi write yang harus atomik.
- API hanya memetakan error menjadi respons aman. Detail SQL, token, URL database, atau kesalahan runtime tidak boleh dikirim ke client.
- Pecah file Rust yang mulai melewati sekitar 800 baris, dan pertahankan visibility minimum.

## Konvensi dashboard

- Gunakan Vue 3 Composition API dengan `<script setup lang="ts">`.
- HTTP berada di `src/lib/api/<domain>.ts`; state dan orkestrasi reusable berada di composable atau Pinia setup store.
- Pakai token Tailwind semantik seperti `bg-background`, `text-foreground`, dan `border-border`.
- Gunakan komponen dari `src/components/ui/`, `cn()`, dan ikon `@lucide/vue`.
- Semua route yang membutuhkan data harus melewati `apiFetch`, sehingga Bearer token, refresh, timeout, dan header request state-changing konsisten.

## Perubahan dokumentasi

`docs/` adalah sumber konten untuk dokumentasi publik yang di-host oleh situs
marketing Ignitify terpisah. Repository ini sengaja bukan project VitePress.

Dokumentasi menggunakan path berbasis file. Tambahkan halaman Markdown English
di `docs/` dan terjemahan Indonesia pada path yang sesuai di `docs/id/`.
Pertahankan heading, link, dan referensi aset relatif agar repository marketing
dapat memakai konten ini sebagai root dokumentasi VitePress-nya. Perbarui
konfigurasi VitePress, sidebar locale, navigasi, dan build dokumentasi di
repository marketing.

Jangan menuliskan token, password, identity age, atau isi `.env` yang nyata ke dokumen, contoh, snapshot, maupun screenshot.
