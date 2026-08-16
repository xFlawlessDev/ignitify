# Benchmark Footprint Baseline

Halaman ini mencatat baseline footprint idle yang dapat diulang untuk Ignitify
dan Dokploy. Ini adalah bukti dari satu pengujian terkontrol, bukan klaim
performa universal.

## Cakupan

- Tanggal: 2026-08-15.
- Host: mesin virtual Ubuntu 24.04 baru yang terpisah, masing-masing satu vCPU.
- Platform: Ignitify `v0.2.0` dan Dokploy `v0.30.0`.
- Status: instalasi selesai, endpoint health lokal merespons, tanpa project,
  aplikasi, atau deployment yang dibuat pengguna.
- Sampling: 12 sampel pada interval lima detik (total 60 detik), diambil secara
  paralel setelah kedua control plane sehat.
- Akses: host key SSH diverifikasi sebelum setiap pengukuran. Record ini tidak
  mengumpulkan credential maupun data aplikasi.

Kapasitas host awal tidak sama: Ignitify memakai host 1 GiB dan Dokploy memakai
host 2 GiB. Snapshot instalasi historis tersebut tetap disimpan di bawah, tetapi
hasil ternormalisasi 1 vCPU / 2 GiB menjadi perbandingan utama.

## Baseline Idle Ternormalisasi

Baseline utama berikut diambil setelah kedua host dinormalisasi menjadi satu
vCPU dan 1.967 MiB RAM. Kedua platform lebih dahulu dikembalikan ke state baru
tanpa workload. Dua belas sampel lima detik dikumpulkan paralel setelah control
plane stabil. CPU host memakai delta dari `/proc/stat`; memori container adalah
jumlah seluruh container produk yang sedang berjalan.

| Metrik | Ignitify | Dokploy |
| --- | ---: | ---: |
| Memori host | 1.967 MiB | 1.967 MiB |
| CPU host rata-rata | 2,17% | 3,57% |
| Memori tersedia rata-rata | 1.539 MiB | 769 MiB |
| Container platform berjalan | 3 | 3 |
| Docker image | 3 | 4 |
| Penyimpanan Docker image | 350,8 MB | 3,911 GB |
| Memori container platform rata-rata | 168,3 MiB | 816,4 MiB |
| Proses control plane khusus rata-rata | 11,5 MiB | N/A (berbasis container) |
| Total control plane terukur | sekitar 179,8 MiB | sekitar 816,4 MiB |
| Endpoint health lokal | HTTP 200 | HTTP 200 |

Total container Ignitify terdiri dari Traefik (sekitar 108,9 MiB), Docker read
proxy (sekitar 10,2 MiB), dan ingress fallback (sekitar 49,2 MiB). Total
Dokploy terdiri dari aplikasi Dokploy (sekitar 737,2 MiB), PostgreSQL (sekitar
62,7 MiB), dan Traefik Dokploy (sekitar 17,1 MiB). Nilai ini adalah observasi
pada pasangan host tersebut, bukan jaminan kapasitas.

## Baseline Instalasi Historis

| Metrik | Ignitify | Dokploy |
| --- | ---: | ---: |
| Memori host | 961 MiB | 1.967 MiB |
| CPU host rata-rata | 1,82% | 2,04% |
| Endpoint health | HTTP 200 | HTTP 307 redirect |
| Container berjalan | 3 | 2 |
| Docker image | 3 | 2 |
| Penyimpanan Docker image | 350,8 MB | 3,677 GB |
| Proses control plane khusus | 56,2 MiB | N/A (berbasis container) |
| Memori container platform | 26,3 MiB rata-rata | 995,8 MiB rata-rata cgroup |
| Memori aplikasi dan database stabil | N/A | 830,1 MiB + 61,3 MiB |

Proses control plane Ignitify yang terukur bersama container platform-nya
sekitar 82,5 MiB. Sampel warm-up Dokploy yang berulang stabil di sekitar 891,4
MiB untuk container aplikasi dan PostgreSQL. Rata-rata cgroup dapat mencakup
efek accounting memori seperti page cache, sehingga pengukuran per-container
tetap dicatat bersamanya.

Nilai host path Ignitify 98 MiB dan Dokploy 1 MiB sengaja tidak dimasukkan ke
tabel: kedua produk menyimpan porsi data yang berbeda pada storage yang dikelola
Docker sehingga path tersebut tidak sebanding.

## Evidence Workload Ternormalisasi

Run workload utama memakai spesifikasi Compose raw yang sama pada setiap host
ternormalisasi: satu image `nginx:1.27.5-alpine` yang dipin ke
`sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10`.
Tidak ada port atau domain publik, repository eksternal, secret, maupun data
aplikasi. Deployment dikirim melalui API masing-masing produk, dan readiness
mewajibkan request HTTP dari dalam container Nginx yang berjalan ke
`127.0.0.1:80`.

Setiap platform menerima tiga sampel cold dan tiga sampel warm. Sebelum setiap
sampel cold, harness hanya menghapus container Nginx benchmark yang
berlabel/teridentifikasi serta image Nginx cache-nya; sampel warm mempertahankan
cache image tersebut. Ini mengendalikan status image pull tanpa melewati alur
deployment produk. Environment production default Ignitify memerlukan approval
eksplisit segera setelah submission. Alur raw Compose Dokploy tidak memiliki
aksi approval yang setara.

| Evidence | Ignitify | Dokploy |
| --- | ---: | ---: |
| Respons API setup | bootstrap 200, project 201, service 201 | sign-up 200, project 200, environment 200, Compose 200 |
| Approval | diperlukan; seluruh respons approval 202 | tidak digunakan dalam Compose run ini |
| Sampel cold submit hingga sehat | 10.477 / 10.402 / 37.399 ms | 7.606 / 6.464 / 7.542 ms |
| Median cold | 10.477 ms | 7.542 ms |
| Rata-rata cold | 19.426 ms | 7.204 ms |
| Sampel warm submit hingga sehat | 2.185 / 2.215 / 2.205 ms | 1.286 / 337 / 299 ms |
| Median warm | 2.205 ms | 337 ms |
| Rata-rata warm | 2.202 ms | 641 ms |
| Ignitify approval hingga sehat | cold: 10.447 / 10.352 / 37.369 ms; warm: 2.150 / 2.184 / 2.145 ms | N/A |
| Pemeriksaan HTTP dalam container | lulus untuk seluruh enam sampel | lulus untuk seluruh enam sampel |
| Memori container Nginx | 2,355-2,414 MiB | 2,367-2,398 MiB |
| Storage Docker image dengan workload | 425,2 MB | 3,911 GB |
| Restart control plane hingga API sehat | 1.137 ms; HTTP workload tetap lulus | 43.231 ms; HTTP workload tetap lulus |

Sampel cold Ignitify 37.399 ms mencapai `healthy` dengan sukses, namun merupakan
outlier yang material. Nilai tersebut dipertahankan, bukan dibuang. Pengukuran
Ignitify menghitung dari submission hingga state sehat yang dikonfirmasi worker;
Dokploy menghitung dari submission hingga container Nginx berjalan dan sehat
secara internal.

Harness benchmark memperlihatkan dua keterbatasan cleanup yang relevan bagi
evidence. Endpoint stop Ignitify mengembalikan 202 setelah deployment sehat,
namun tidak menghapus container workload dalam window observasi 120 detik;
penghapusan service/project berikutnya mengembalikan 409. Container test yang
berlabel tepat kemudian dihapus dan state aplikasi test-only di-reset. Enam
sampel timing Dokploy selesai, tetapi harness gagal saat memformat field memori
sebelum cleanup API; workload tetap sehat saat control plane restart diukur,
lalu container Nginx yang tepat dan data aplikasi test-only dihapus. Kedua host
tidak menyisakan workload benchmark.

Host key SSH diverifikasi sebelum setiap sesi. Output dibatasi pada status HTTP,
state deployment terminal, waktu, storage agregat, dan memori. Credential,
token, password yang digenerate, project ID, log, serta nilai konfigurasi hanya
berada pada file sementara dengan permission terbatas dan tidak dicatat di sini.

Ini adalah pengukuran berulang pada host dengan kapasitas setara, tetapi tetap
bukan ranking kecepatan platform universal. Pengujian memakai satu image kecil,
satu pasangan host, eksekusi serial, readiness lokal dari dalam container, serta
produk dengan semantik approval dan restart berbeda. Latensi ingress publik,
throughput, dan perilaku gagal di bawah beban belum diukur.

## Evidence Workload Historis

Pengujian satu kali berikut memakai spesifikasi Compose raw yang sama pada kedua
host. Spesifikasi tersebut hanya berisi image
`nginx:1.27.5-alpine` yang dipin ke
`sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10`.
Tidak ada port atau domain publik, repository eksternal, secret, maupun data
aplikasi. Readiness diperiksa dengan request HTTP dari dalam container yang
berjalan ke `127.0.0.1:80`.

| Evidence | Ignitify | Dokploy |
| --- | ---: | ---: |
| Respons API create | bootstrap 200, project 201, service 201, deploy 202 | register 200, project 200, environment 200, Compose 200, deploy 200 |
| Approval | diperlukan dan disetujui, 202 | tidak digunakan dalam Compose run ini |
| Submit hingga sehat | 11.509 ms | 8.936 ms |
| Approval hingga sehat | 11.455 ms | N/A |
| Pemeriksaan HTTP dalam container | pass | pass |
| Memori container Nginx | 2,512 MiB | 2,414 MiB |
| Storage Docker image setelah deployment | 425,2 MB | 3,911 GB |
| Cleanup API produk | stop/delete awal beradu dengan proses asinkron dan memberi 409; residual terverifikasi dihapus saat reset test | Compose delete 200, project remove 200 |

Setiap koneksi SSH memverifikasi host key ED25519 yang diharapkan sebelum
menjalankan pengukuran. Output dibatasi pada status HTTP, status deployment
terminal, waktu, storage Docker agregat, dan memori container. Credential,
token, password yang digenerate, project ID, log, dan nilai konfigurasi hanya
berada pada file sementara dengan permission terbatas dan tidak dicatat di sini.

Waktu ini adalah evidence satu deployment image cold pada host yang kapasitasnya
berbeda. Ini bukan ranking kecepatan platform yang valid: Ignitify juga
menjalankan workflow approval production, sedangkan kapasitas memori kedua host
berbeda.

## Interpretasi

Pada host ternormalisasi, Ignitify memakai sekitar 636,6 MiB lebih sedikit
memori control plane idle terukur dan menyisakan sekitar 770 MiB memori host
lebih banyak. Storage Docker image instalasinya sekitar 3,56 GB lebih rendah.
Ini adalah evidence kuat untuk footprint idle yang lebih kecil pada VPS 2 GiB;
hal ini tidak mengukur kesetaraan fitur, throughput aplikasi berkelanjutan,
latensi request publik, maupun keandalan di bawah beban.

Dokploy menyelesaikan alur submission hingga sehat yang persis diuji lebih
cepat pada run ini, khususnya saat cache image warm. Hasil ini harus tetap
menjadi observasi spesifik workflow, bukan klaim performa seluruh produk:
Ignitify memiliki transisi approval production, kontrak readiness tidak sama,
dan hanya satu pasangan host serta image yang dipakai. Restart control plane
Dokploy yang jauh lebih lama juga merupakan evidence untuk metode restart ini,
bukan kesimpulan availability umum.

Dokploy dievaluasi pada 2 GiB karena panduan instalasinya menetapkan minimal 2
GB memori. Percobaan awal pada 1 GiB tidak dipertahankan sebagai hasil benchmark:
host sudah di-resize sebelum hasil stabil dapat ditetapkan.

## Metode Lanjutan

Benchmark berikutnya harus lebih dahulu menyelesaikan dan memberi regression
test pada jalur stop/cleanup service sehat yang terlihat di atas. Setelah itu,
ulangi workload sama pada beberapa pasangan host baru dan catat:

1. Sampel cold dan warm submission hingga sehat dengan median serta sebaran.
2. Memori control plane dan workload saat idle serta di bawah beban terkontrol.
3. Latensi HTTP dan error rate melalui hostname ingress nyata yang dialokasikan
   terpisah.
4. Waktu pemulihan reboot seluruh host, storage Docker tersisa, dan keberhasilan
   cleanup melalui API produk.

Jaga pengujian tetap terisolasi dari infrastruktur produksi, gunakan hanya data
sintetis, dan hapus aplikasi sementara setelah pengujian selesai.
