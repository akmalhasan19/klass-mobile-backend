# Media Generation Manual Verification

Dokumen ini dipakai untuk phase 12.3 agar verifikasi smoke dan end-to-end media generation pada arsitektur 3 deployable bisa diulang di local, staging, atau target deploy.

## Prasyarat

1. Backend Laravel, queue worker, llm-adapter-service, dan Python media generator service sudah aktif.
2. Environment media generation sudah terisi, termasuk `MEDIA_GENERATION_LLM_ADAPTER_BASE_URL`, `MEDIA_GENERATION_LLM_ADAPTER_SHARED_SECRET`, `MEDIA_GENERATION_PYTHON_BASE_URL`, dan shared secret Python renderer.
3. Adapter sudah terhubung ke Postgres eksternal dan provider Gemini aktif pada route interpretation dan delivery bila ingin memverifikasi path Gemini end-to-end.
4. Teacher account sudah tersedia.

## Smoke Checks Service Boundary

1. Jalankan `php artisan media-generation:smoke-llm-adapter` dari folder `backend/`.
   Expected: command lolos dengan output `LLM adapter service is reachable and healthy.`, memeriksa `GET /health` dan `GET /v1/health`, serta melaporkan `Postgres ready: yes`.
2. Jika provider aktif memang harus Gemini, jalankan `php artisan media-generation:smoke-llm-adapter --exercise-routes --expect-provider=gemini`.
   Expected: backend berhasil mengirim signed request ke `/v1/interpret` dan `/v1/respond`, output menunjukkan `Interpret smoke provider: gemini` dan `Respond smoke provider: gemini`, serta tidak ada fallback contract.
3. Jalankan `php artisan media-generation:smoke-python-service` dari folder `backend/`.
   Expected: backend dapat reach Python renderer dan health payload renderer lolos.
4. Jalankan `php artisan test --testdox tests/Feature/MediaGenerationDeploymentReadinessTest.php tests/Feature/MediaGenerationOrchestrationServiceTest.php tests/Feature/MediaGenerationPublicationAndDeliveryTest.php tests/Feature/Phase10EndToEndVerificationTest.php`.
   Expected: harness backend untuk health adapter, boundary adapter, delivery contract, dan flow end-to-end lolos.
5. Opsional untuk cek payload mentah adapter dari host target, panggil langsung:
   `curl -fsS "$MEDIA_GENERATION_LLM_ADAPTER_BASE_URL/health"`
   `curl -fsS "$MEDIA_GENERATION_LLM_ADAPTER_BASE_URL/v1/health"`
   Pastikan `dependencies.postgres.ready=true`, `dependencies.providers.interpretation.provider=gemini`, `dependencies.providers.delivery.provider=gemini`, dan `auth.ready=true`.

## Manual End-to-End Flow

1. Login ke aplikasi menggunakan akun teacher/guru.
2. Buka Home dan kirim prompt dari section `Generate Learning Topics`.
   Contoh: `Buatkan deck termodinamika untuk kelas 11 dengan latihan singkat di akhir.`
3. Verifikasi backend membuat record `media_generations` dan worker queue memproses status secara normal.
   Contoh query: `SELECT id, status, llm_provider, llm_model, preferred_output_type, resolved_output_type, error_code FROM media_generations ORDER BY created_at DESC LIMIT 5;`
4. Verifikasi payload interpretasi, keputusan output, dan audit boundary adapter tersimpan.
   Kolom yang perlu dicek: `interpretation_payload`, `interpretation_audit_payload`, `generation_spec_payload`, `decision_payload`.
5. Verifikasi resolved output type mengikuti keputusan sistem atau override teacher.
   Jika request memakai override, pastikan `resolved_output_type` sama dengan override.
6. Verifikasi Python service menerima generation spec tanpa perubahan kontrak dan menghasilkan artifact sesuai format target.
   Cek `generator_service_response.response.artifact_metadata`, `generator_service_response.response.raw_payload`, dan endpoint health Python.
7. Verifikasi artifact ter-upload ke storage dan thumbnail tersedia bila format mendukung.
   Kolom yang perlu dicek: `storage_path`, `file_url`, `thumbnail_url`, `mime_type`.
8. Verifikasi hasil masuk ke Workspace.
   Cek `GET /api/topics?search=<judul hasil>` atau buka daftar workspace di aplikasi.
9. Verifikasi hasil masuk ke Homepage recommendation feed sebagai item `ai_generated`.
   Cek `GET /api/homepage-recommendations` dan pastikan `source_type=ai_generated`.
10. Verifikasi kartu hasil teacher menampilkan CTA `download`, `open`, dan `share`, lalu jalankan ketiganya dari aplikasi.

## Frontend Regression Gate

1. Jalankan `flutter test test/screens/home_screen_media_generation_flow_test.dart test/widgets/media_generation_status_card_test.dart test/screens/home_screen_media_generation_role_test.dart` dari folder `frontend/`.
2. Pastikan polling status card, hydration result payload, dan role boundary teacher tetap lolos.