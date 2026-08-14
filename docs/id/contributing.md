# Panduan kontribusi

Kontribusi harus mempertahankan batas arsitektur dan memperbarui kontrak publik ketika perilaku berubah.

## Sebelum mengubah kode

- Baca `AGENTS.md` pada repository yang diubah.
- Periksa worktree terlebih dahulu; jangan membatalkan perubahan milik orang lain.
- Tentukan layer pemilik perubahan: domain, persistence, control plane, HTTP, dashboard, atau dokumentasi.
- Untuk perubahan antar layer, mulai dari kontrak/domain dan bergerak ke arah UI, bukan sebaliknya.

## Perubahan backend

1. Tambahkan test untuk aturan domain, akses, persistence, atau lifecycle baru.
2. Gunakan migrasi SQL baru bernomor untuk data yang bertahan; jangan mengedit migrasi yang sudah dipakai.
3. Jaga handler sebagai adapter HTTP dan letakkan external side effect di worker/runtime adapter.
4. Map error ke respons aman dan jangan bocorkan detail internal.
5. Jalankan:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Perubahan dashboard

1. Ubah typed client di `src/lib/api/` sebelum view.
2. Tempatkan perilaku reusable dalam composable atau Pinia store.
3. Gunakan token Tailwind semantik dan komponen UI yang ada.
4. Tambahkan spec Vitest untuk perubahan state/API yang bermakna.
5. Dari `ignitify/frontend`, jalankan:

```bash
vp run check
vp run test
vp run build
```

## Perubahan dokumentasi

- Perlakukan `docs/` sebagai sumber konten untuk dokumentasi publik yang
  diterbitkan oleh situs marketing Ignitify. Repository ini tidak memiliki
  aplikasi VitePress maupun deployment-nya.
- Pertahankan halaman English sebagai default dan pasangan Indonesia di bawah
  `docs/id/` tetap sinkron saat menambah atau mengubah konten.
- Pertahankan path berbasis file dan relative link agar repository marketing
  dapat memasang direktori ini langsung ke content root VitePress-nya.
- Perbarui navigasi VitePress, tema, language switcher, dan build dokumentasi
  pada repository marketing, bersama perubahan konten yang membutuhkannya.
- Jangan menambahkan dependency VitePress, konfigurasi, output site hasil
  generate, atau aset situs marketing ke repository control plane ini.

## Template publik

Informasi, perubahan, dan kontribusi template dikelola di [xFlawlessDev/ignitify-templates](https://github.com/xFlawlessDev/ignitify-templates). Gunakan repository tersebut sebagai sumber tunggal untuk template.

## Pull request yang baik

Jelaskan masalah, perubahan perilaku, migration/konfigurasi yang diperlukan, dan verifikasi yang dijalankan. Pisahkan refactor yang tidak relevan dari perubahan fungsional. Jika pengujian tidak dapat dijalankan, catat batasannya secara spesifik daripada mengklaim lolos.
