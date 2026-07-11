# Render API Smoke Test

Dokumen ini dipakai untuk verifikasi backend Laravel setelah deploy ke Render.
Fokusnya adalah memastikan container boot normal, koneksi database hidup, storage URL valid, dan flow auth dasar tidak rusak.

## Prasyarat

- Service Render sudah `Live` dan health check `/up` berstatus sehat.
- `APP_URL` sudah diisi sesuai domain Render.
- `DB_URL` memakai Supabase Session Pooler.
- `FILESYSTEM_DISK` sudah diarahkan ke `supabase`.
- Data seed minimum tersedia untuk `topics`, `contents`, dan `marketplace_tasks`.
- Siapkan satu email uji baru untuk flow registrasi.

## Variabel Uji

Gunakan base URL sesuai service Render Anda.

```bash
BASE_URL="https://your-service-name.onrender.com"
TEST_NAME="Render QA User"
TEST_EMAIL="render-smoke-$(date +%s)@example.com"
TEST_PASSWORD="Password123!"
```

Jika menguji dari PowerShell, Anda bisa set nilai yang sama dengan:

```powershell
$BASE_URL = "https://your-service-name.onrender.com"
$TEST_NAME = "Render QA User"
$TEST_EMAIL = "render-smoke-$(Get-Date -Format yyyyMMddHHmmss)@example.com"
$TEST_PASSWORD = "Password123!"
```

## Smoke Test Dasar

### 1. Health Check

```bash
curl -fsS "$BASE_URL/up"
```

Expected:

- HTTP `200`
- Halaman health Render Laravel muncul tanpa error.

### 2. Public Topics

```bash
curl -fsS "$BASE_URL/api/topics?per_page=5"
```

Expected:

- JSON `success: true`
- Data berada di `data`
- Metadata pagination berada di `meta`

### 3. Public Contents

```bash
curl -fsS "$BASE_URL/api/contents?per_page=5"
```

Expected:

- HTTP `200`
- Minimal satu item atau empty state JSON yang valid

### 4. Gallery

```bash
curl -fsS "$BASE_URL/api/gallery?per_page=5"
```

Expected:

- HTTP `200`
- `media_url` yang terisi mengarah ke URL storage publik

### 5. Marketplace Tasks

```bash
curl -fsS "$BASE_URL/api/marketplace-tasks?per_page=5"
```

Expected:

- HTTP `200`
- Relasi `content` termuat sesuai resource

## Smoke Test Auth

### 6. Register

```bash
curl -fsS -X POST "$BASE_URL/api/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"name\":\"$TEST_NAME\",\"email\":\"$TEST_EMAIL\",\"password\":\"$TEST_PASSWORD\",\"password_confirmation\":\"$TEST_PASSWORD\"}"
```

Expected:

- HTTP `201`
- JSON `success: true`
- Token berada di `data.token`

### 7. Login

```bash
curl -fsS -X POST "$BASE_URL/api/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"$TEST_EMAIL\",\"password\":\"$TEST_PASSWORD\"}"
```

Expected:

- HTTP `200`
- JSON `success: true`
- Salin nilai `data.token` untuk langkah berikutnya

Simpan token ke variabel shell:

```bash
TOKEN="paste-token-here"
```

### 8. Current User

```bash
curl -fsS "$BASE_URL/api/auth/me" \
  -H "Authorization: Bearer $TOKEN"
```

Expected:

- HTTP `200`
- User yang kembali sesuai akun hasil register/login

### 9. Logout

```bash
curl -fsS -X POST "$BASE_URL/api/auth/logout" \
  -H "Authorization: Bearer $TOKEN"
```

Expected:

- HTTP `200`
- Request berikutnya ke `/api/auth/me` dengan token yang sama harus gagal `401`

## Smoke Test Error Handling

### 10. Route Tidak Ada

```bash
curl -i "$BASE_URL/api/not-found"
```

Expected:

- HTTP `404`
- JSON `success: false`
- Message: `Endpoint tidak ditemukan.`

### 11. Protected Route Tanpa Token

```bash
curl -i "$BASE_URL/api/auth/me"
```

Expected:

- HTTP `401`
- JSON `success: false`

## Smoke Test Upload Opsional

Gunakan hanya jika storage Supabase sudah final.
Field multipart untuk avatar harus bernama `file`.

```bash
curl -fsS -X POST "$BASE_URL/api/user/avatar" \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@avatar-test.png"
```

Expected:

- HTTP `200` atau `201`
- URL avatar yang kembali mengarah ke bucket publik Supabase
- File tetap bisa diakses setelah refresh data user

## Release Gate

Sebelum menandai deploy aman, pastikan semua poin ini terpenuhi:

- `/up` sehat selama beberapa request beruntun
- Endpoint read utama tidak menghasilkan `500`
- Flow register, login, me, logout lolos
- Log Render tidak menunjukkan exception berulang
- Tidak ada file upload yang bergantung pada local disk Render

## Catatan

- Jika endpoint public berhasil di lokal Docker tetapi gagal di Render, cek ulang env `DB_URL`, `APP_URL`, dan credential storage.
- Jika auth berhasil tetapi upload gagal, fokus cek `FILESYSTEM_DISK`, endpoint Supabase S3, dan bucket visibility.
- Jika response hanya menampilkan message generik, baca log service di Render karena handler API memang menyembunyikan detail exception dari client.