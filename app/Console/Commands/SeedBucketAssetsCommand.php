<?php

namespace App\Console\Commands;

use App\Services\FileUploadService;
use Illuminate\Console\Command;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;

/**
 * SeedBucketAssetsCommand
 *
 * Upload dummy asset files dari direktori lokal ke Supabase Storage bucket.
 * File-file ini dipakai oleh seeder untuk mengisi URL bucket ke tabel-tabel terkait.
 *
 * Assets yang di-upload:
 * - gallery/    → gambar project (ppt_design_3.jpg, infographic, math preview)
 * - avatars/    → foto profil freelancer/teacher (agus, ani, budi, susi)
 * - materials/  → placeholder material file
 *
 * Usage: php artisan seed:bucket-assets
 */
class SeedBucketAssetsCommand extends Command
{
    protected $signature = 'seed:bucket-assets {--force : Overwrite existing files}';
    protected $description = 'Upload seed asset files ke Supabase Storage bucket';

    /**
     * Definisi asset yang akan di-upload.
     * Key = path tujuan di bucket, Value = path sumber lokal relatif ke frontend/assets
     */
    protected array $assets = [
        // Gallery / project images
        'gallery/ppt_design_3.jpg' => 'images/ppt_design_3.jpg',
        'gallery/infographic_preview_health.png' => 'images/infographic_preview_health_1773981088610.png',
        'gallery/square_preview_math.png' => 'images/square_preview_math_1773981103817.png',

        // Avatar images
        'avatars/agus.png' => 'avatars/agus.png',
        'avatars/ani.png' => 'avatars/ani.png',
        'avatars/budi.png' => 'avatars/budi.png',
        'avatars/susi.png' => 'avatars/susi.png',
    ];

    public function handle(FileUploadService $uploadService): int
    {
        $this->info('🚀 Mulai upload seed assets ke Supabase Storage...');
        $this->newLine();

        $assetsDir = base_path('../frontend/assets');

        if (!is_dir($assetsDir)) {
            $this->error("❌ Direktori frontend assets tidak ditemukan: {$assetsDir}");
            return self::FAILURE;
        }

        $disk = Storage::disk('supabase');
        $uploaded = 0;
        $skipped = 0;
        $failed = 0;

        foreach ($this->assets as $bucketPath => $localPath) {
            $fullLocalPath = $assetsDir . '/' . $localPath;

            if (!file_exists($fullLocalPath)) {
                $this->warn("⚠️  File tidak ditemukan: {$localPath} — dilewati.");
                $skipped++;
                continue;
            }

            // Check apakah file sudah ada di bucket
            if (!$this->option('force') && $disk->exists($bucketPath)) {
                $this->line("  ⏩ Sudah ada: <comment>{$bucketPath}</comment>");
                $skipped++;
                continue;
            }

            try {
                $disk->put($bucketPath, file_get_contents($fullLocalPath), 'public');
                $publicUrl = $uploadService->generatePublicUrl($bucketPath);
                $this->line("  ✅ Uploaded: <info>{$bucketPath}</info>");
                $this->line("     URL: <comment>{$publicUrl}</comment>");
                $uploaded++;
            } catch (\Throwable $e) {
                $this->error("  ❌ Gagal upload {$bucketPath}: {$e->getMessage()}");
                $failed++;
            }
        }

        $this->newLine();
        $this->info("📊 Hasil: {$uploaded} uploaded, {$skipped} skipped, {$failed} failed");

        return $failed > 0 ? self::FAILURE : self::SUCCESS;
    }
}
