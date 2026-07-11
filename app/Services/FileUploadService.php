<?php

namespace App\Services;

use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Str;
use InvalidArgumentException;

/**
 * FileUploadService
 *
 * Menangani upload file ke Supabase Storage melalui S3-compatible API.
 * Bertanggung jawab atas:
 * - Sanitasi filename (lowercase, slug, hapus karakter spesial)
 * - Anti-collision strategy (timestamp + random string prefix)
 * - Validasi upload (mime type + max size per kategori)
 * - Generate public URL dari file yang di-upload
 */
class FileUploadService
{
    /**
     * Disk name yang dipakai untuk Supabase Storage.
     */
    protected string $disk = 'supabase';

    /**
     * Upload file ke Supabase Storage bucket pada kategori tertentu.
     *
     * @param  UploadedFile  $file      File yang di-upload
     * @param  string        $category  Kategori upload (avatars, gallery, materials, attachments)
     * @return array{path: string, url: string}
     *
     * @throws \InvalidArgumentException  Jika kategori tidak valid
     * @throws \Illuminate\Validation\ValidationException  Jika validasi file gagal
     */
    public function upload(UploadedFile $file, string $category): array
    {
        $this->validateCategory($category);
        $this->validateFile($file, $category);

        $categoryConfig = $this->getCategoryConfig($category);
        $sanitizedName = $this->sanitizeFilename($file);
        $path = $categoryConfig['path'] . '/' . $sanitizedName;

        // Upload ke Supabase Storage via S3-compatible driver
        Storage::disk($this->disk)->put($path, file_get_contents($file->getRealPath()), 'public');

        return [
            'path' => $path,
            'url' => $this->generatePublicUrl($path),
        ];
    }

    /**
     * Upload file lokal yang sudah ada di filesystem ke bucket kategori tertentu.
     *
     * @return array{path: string, url: string}
     */
    public function uploadFromPath(string $filePath, string $originalName, string $category): array
    {
        if (! is_file($filePath)) {
            throw new InvalidArgumentException("File lokal '{$filePath}' tidak ditemukan.");
        }

        $uploadedFile = new UploadedFile(
            $filePath,
            $originalName,
            mime_content_type($filePath) ?: null,
            null,
            true,
        );

        return $this->upload($uploadedFile, $category);
    }

    /**
     * Hapus file dari Supabase Storage bucket.
     *
     * @param  string  $path  Path relatif file di bucket
     * @return bool
     */
    public function delete(string $path): bool
    {
        return Storage::disk($this->disk)->delete($path);
    }

    /**
     * Check apakah file ada di bucket.
     *
     * @param  string  $path  Path relatif file di bucket
     * @return bool
     */
    public function exists(string $path): bool
    {
        return Storage::disk($this->disk)->exists($path);
    }

    /**
     * Sanitasi filename:
     * - Lowercase
     * - Hapus karakter spesial (hanya alfanumerik, dash, underscore, dot)
     * - Prefix dengan timestamp + 8 karakter random untuk anti-collision
     *
     * Contoh output: "1711766400_a3b8f2e1_laporan-tugas.pdf"
     */
    protected function sanitizeFilename(UploadedFile $file): string
    {
        $extension = strtolower($file->getClientOriginalExtension());

        // Ambil nama file tanpa extension, lalu slugify
        $nameWithoutExt = pathinfo($file->getClientOriginalName(), PATHINFO_FILENAME);
        $slug = Str::slug($nameWithoutExt, '-');

        // Fallback jika nama file hanya karakter spesial
        if (empty($slug)) {
            $slug = 'file';
        }

        // Prefix: unix timestamp + 8 char random string
        $prefix = time() . '_' . Str::random(8);

        return "{$prefix}_{$slug}.{$extension}";
    }

    /**
     * Validasi bahwa kategori termasuk di daftar yang diizinkan.
     *
     * @throws \InvalidArgumentException
     */
    protected function validateCategory(string $category): void
    {
        $categories = config('filesystems.upload_categories', []);

        if (!array_key_exists($category, $categories)) {
            $allowed = implode(', ', array_keys($categories));
            throw new InvalidArgumentException(
                "Kategori upload '{$category}' tidak valid. Kategori yang diizinkan: {$allowed}"
            );
        }
    }

    /**
     * Validasi file terhadap aturan kategori (mime type + max size).
     *
     * @throws \Illuminate\Validation\ValidationException
     */
    protected function validateFile(UploadedFile $file, string $category): void
    {
        $config = $this->getCategoryConfig($category);
        $extension = strtolower($file->getClientOriginalExtension());
        $sizeKb = $file->getSize() / 1024;

        // Validasi mime type berdasarkan extension
        if (!in_array($extension, $config['allowed_mimes'])) {
            $allowed = implode(', ', $config['allowed_mimes']);
            throw \Illuminate\Validation\ValidationException::withMessages([
                'file' => "Tipe file '.{$extension}' tidak diizinkan untuk kategori '{$category}'. Tipe yang diizinkan: {$allowed}",
            ]);
        }

        // Validasi ukuran file
        if ($sizeKb > $config['max_size_kb']) {
            $maxMb = round($config['max_size_kb'] / 1024, 1);
            throw \Illuminate\Validation\ValidationException::withMessages([
                'file' => "Ukuran file melebihi batas maksimal {$maxMb} MB untuk kategori '{$category}'.",
            ]);
        }
    }

    /**
     * Ambil konfigurasi untuk kategori tertentu.
     *
     * @return array{path: string, allowed_mimes: string[], max_size_kb: int}
     */
    protected function getCategoryConfig(string $category): array
    {
        return config("filesystems.upload_categories.{$category}");
    }

    /**
     * Generate public URL untuk file di Supabase Storage.
     *
     * Format: {SUPABASE_URL}/storage/v1/object/public/{bucket}/{path}
     */
    public function generatePublicUrl(string $path): string
    {
        $bucket = (string) config('filesystems.disks.supabase.bucket', 'klass-storage');
        $publicBaseUrl = trim((string) config('filesystems.disks.supabase.public_base_url', ''));

        if ($publicBaseUrl !== '') {
            return rtrim($publicBaseUrl, '/') . "/storage/v1/object/public/{$bucket}/{$path}";
        }

        $endpoint = rtrim((string) config('filesystems.disks.supabase.endpoint', ''), '/');

        if ($endpoint !== '') {
            $publicEndpoint = preg_replace('#/storage/v1/s3/?$#', '/storage/v1/object/public', $endpoint) ?: $endpoint;

            return "{$publicEndpoint}/{$bucket}/{$path}";
        }

        return "/storage/v1/object/public/{$bucket}/{$path}";
    }
}
