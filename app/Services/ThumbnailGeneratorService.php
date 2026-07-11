<?php

namespace App\Services;

use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Str;
use Throwable;

/**
 * ThumbnailGeneratorService
 *
 * Mengekstrak halaman/slide/page pertama dari file dokumen (PDF, PPTX, DOCX)
 * dan mengkonversinya menjadi gambar thumbnail PNG.
 *
 * Strategi:
 * - PDF  → Imagick (render page pertama)
 * - PPTX → ZipArchive (extract thumbnail dari docProps/ atau slide image pertama)
 * - DOCX → ZipArchive (extract thumbnail dari docProps/ atau gambar pertama)
 *
 * Semua method mengembalikan path ke temporary file PNG, atau null jika gagal.
 * Caller bertanggung jawab untuk menghapus temp file setelah selesai.
 */
class ThumbnailGeneratorService
{
    /**
     * Generate thumbnail dari file dokumen.
     *
     * @param  string  $filePath  Absolute path ke file lokal
     * @return string|null  Absolute path ke file thumbnail (PNG/JPG), atau null jika gagal
     */
    public function generateFromFile(string $filePath): ?string
    {
        if (! file_exists($filePath)) {
            return null;
        }

        $extension = strtolower(pathinfo($filePath, PATHINFO_EXTENSION));

        try {
            return match ($extension) {
                'pdf' => $this->generateFromPdf($filePath),
                'pptx' => $this->generateFromPptx($filePath),
                'ppt' => $this->generateFromPptx($filePath), // Old .ppt may not work, but try
                'docx' => $this->generateFromDocx($filePath),
                'doc' => null, // Old .doc format not supported (binary format)
                default => null,
            };
        } catch (Throwable $e) {
            report($e);
            return null;
        }
    }

    /**
     * Generate thumbnail dari file yang ada di Supabase Storage.
     * Download file ke temp directory, generate thumbnail, lalu hapus temp file.
     *
     * @param  string  $storageUrl  Full public URL file di Supabase Storage
     * @return string|null  Absolute path ke thumbnail temp file, atau null jika gagal
     */
    public function generateFromUrl(string $storageUrl): ?string
    {
        try {
            // Download file ke temporary location
            $tempDir = sys_get_temp_dir();
            $extension = strtolower(pathinfo(parse_url($storageUrl, PHP_URL_PATH), PATHINFO_EXTENSION));

            if (empty($extension)) {
                return null;
            }

            $tempFile = $tempDir . '/' . 'thumbnail_src_' . Str::random(12) . '.' . $extension;

            // Download via HTTP
            $contents = @file_get_contents($storageUrl);
            if ($contents === false) {
                return null;
            }

            file_put_contents($tempFile, $contents);

            $thumbnailPath = $this->generateFromFile($tempFile);

            // Cleanup source temp file
            @unlink($tempFile);

            return $thumbnailPath;
        } catch (Throwable $e) {
            report($e);
            return null;
        }
    }

    public function generateFallbackVisual(string $documentType, ?string $title = null): ?string
    {
        $palette = $this->fallbackPalette($documentType);
        $label = strtoupper(trim($documentType) !== '' ? $documentType : 'file');
        $titleText = $this->escapeSvgText($this->truncateText($title ?: 'Generated learning material', 56));
        $tempOutput = sys_get_temp_dir() . '/thumb_fallback_' . Str::random(12) . '.svg';

        $svg = <<<SVG
<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720" fill="none">
  <rect width="1280" height="720" fill="{$palette['background']}"/>
  <rect x="56" y="56" width="1168" height="608" rx="36" fill="{$palette['surface']}" stroke="{$palette['border']}" stroke-width="4"/>
  <rect x="112" y="112" width="168" height="56" rx="28" fill="{$palette['accent']}"/>
  <text x="196" y="148" fill="#FFFFFF" font-family="Segoe UI, Arial, sans-serif" font-size="24" font-weight="700" text-anchor="middle">{$label}</text>
  <text x="112" y="252" fill="#102A43" font-family="Segoe UI, Arial, sans-serif" font-size="48" font-weight="700">{$titleText}</text>
  <text x="112" y="316" fill="#52606D" font-family="Segoe UI, Arial, sans-serif" font-size="28">Preview is unavailable, but the generated file is ready to open.</text>
  <text x="112" y="380" fill="#52606D" font-family="Segoe UI, Arial, sans-serif" font-size="24">Use this card as a fallback visual in Workspace and Homepage surfaces.</text>
  <rect x="112" y="452" width="1056" height="132" rx="24" fill="{$palette['panel']}"/>
  <text x="152" y="512" fill="#102A43" font-family="Segoe UI, Arial, sans-serif" font-size="26" font-weight="600">Fallback thumbnail</text>
  <text x="152" y="558" fill="#486581" font-family="Segoe UI, Arial, sans-serif" font-size="22">Generated automatically because the original document preview could not be extracted.</text>
</svg>
SVG;

        try {
            file_put_contents($tempOutput, $svg);

            return $tempOutput;
        } catch (Throwable $e) {
            report($e);
            @unlink($tempOutput);

            return null;
        }
    }

    /**
     * PDF → Thumbnail menggunakan Imagick.
     * Render halaman pertama sebagai PNG 800x600.
     */
    protected function generateFromPdf(string $filePath): ?string
    {
        if (! extension_loaded('imagick')) {
            return null;
        }

        $tempOutput = sys_get_temp_dir() . '/thumb_' . Str::random(12) . '.png';

        try {
            $imagick = new \Imagick();
            $imagick->setResolution(150, 150);
            $imagick->readImage($filePath . '[0]'); // Page pertama
            $imagick->setImageFormat('png');

            // Resize to max 800x600, maintaining aspect ratio
            $imagick->thumbnailImage(800, 600, true);

            // Set white background (PDF pages can have transparency)
            $background = new \Imagick();
            $background->newImage(
                $imagick->getImageWidth(),
                $imagick->getImageHeight(),
                new \ImagickPixel('white')
            );
            $background->compositeImage($imagick, \Imagick::COMPOSITE_OVER, 0, 0);
            $background->setImageFormat('png');

            $background->writeImage($tempOutput);

            $imagick->clear();
            $imagick->destroy();
            $background->clear();
            $background->destroy();

            return $tempOutput;
        } catch (Throwable $e) {
            report($e);
            @unlink($tempOutput);
            return null;
        }
    }

    /**
     * PPTX → Thumbnail menggunakan ZipArchive.
     *
     * Strategi (urutan prioritas):
     * 1. docProps/thumbnail.jpeg — embedded thumbnail (dihasilkan oleh PowerPoint)
     * 2. ppt/media/image1.* — gambar pertama dalam slide
     */
    protected function generateFromPptx(string $filePath): ?string
    {
        return $this->extractFromOfficeXml($filePath, [
            'docProps/thumbnail.jpeg',
            'docProps/thumbnail.png',
        ], 'ppt/media/');
    }

    /**
     * DOCX → Thumbnail menggunakan ZipArchive.
     *
     * Strategi (urutan prioritas):
     * 1. docProps/thumbnail.jpeg — embedded thumbnail
     * 2. word/media/image1.* — gambar pertama dalam dokumen
     */
    protected function generateFromDocx(string $filePath): ?string
    {
        return $this->extractFromOfficeXml($filePath, [
            'docProps/thumbnail.jpeg',
            'docProps/thumbnail.png',
        ], 'word/media/');
    }

    /**
     * Mengekstrak thumbnail dari format Office XML (PPTX/DOCX).
     *
     * @param  string    $filePath        Path ke file .pptx/.docx
     * @param  string[]  $priorityPaths   Daftar path prioritas untuk thumbnail
     * @param  string    $mediaPrefix     Prefix folder media (ppt/media/ atau word/media/)
     * @return string|null  Path ke temp file gambar, atau null
     */
    protected function extractFromOfficeXml(string $filePath, array $priorityPaths, string $mediaPrefix): ?string
    {
        if (! class_exists(\ZipArchive::class)) {
            return null;
        }

        $zip = new \ZipArchive();

        if ($zip->open($filePath) !== true) {
            return null;
        }

        try {
            // 1. Coba extract dari priority paths terlebih dahulu
            foreach ($priorityPaths as $priorityPath) {
                $contents = $zip->getFromName($priorityPath);
                if ($contents !== false) {
                    $extension = pathinfo($priorityPath, PATHINFO_EXTENSION);
                    $tempOutput = sys_get_temp_dir() . '/thumb_' . Str::random(12) . '.' . $extension;
                    file_put_contents($tempOutput, $contents);
                    $zip->close();
                    return $tempOutput;
                }
            }

            // 2. Fallback: cari gambar pertama di media folder
            $mediaImages = [];
            for ($i = 0; $i < $zip->numFiles; $i++) {
                $name = $zip->getNameIndex($i);
                if ($name === false) {
                    continue;
                }

                // Hanya ambil file di media folder yang merupakan gambar
                if (str_starts_with($name, $mediaPrefix) && $this->isImageExtension($name)) {
                    $mediaImages[] = $name;
                }
            }

            // Urutkan agar image1 lebih dulu dari image2, dst
            sort($mediaImages);

            if (! empty($mediaImages)) {
                $imageName = $mediaImages[0];
                $contents = $zip->getFromName($imageName);
                if ($contents !== false) {
                    $extension = pathinfo($imageName, PATHINFO_EXTENSION);
                    $tempOutput = sys_get_temp_dir() . '/thumb_' . Str::random(12) . '.' . $extension;
                    file_put_contents($tempOutput, $contents);
                    $zip->close();
                    return $tempOutput;
                }
            }

            $zip->close();
            return null;
        } catch (Throwable $e) {
            $zip->close();
            report($e);
            return null;
        }
    }

    /**
     * Cek apakah file memiliki extension gambar yang umum.
     */
    protected function isImageExtension(string $filename): bool
    {
        $ext = strtolower(pathinfo($filename, PATHINFO_EXTENSION));

        return in_array($ext, ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'tiff', 'emf', 'wmf']);
    }

    /**
     * @return array{background: string, surface: string, border: string, accent: string, panel: string}
     */
    protected function fallbackPalette(string $documentType): array
    {
        return match (strtolower(trim($documentType))) {
            'pdf' => [
                'background' => '#FFF4F2',
                'surface' => '#FFFFFF',
                'border' => '#F7C9C3',
                'accent' => '#C0392B',
                'panel' => '#FDECE8',
            ],
            'pptx' => [
                'background' => '#FFF7ED',
                'surface' => '#FFFFFF',
                'border' => '#FBD38D',
                'accent' => '#DD6B20',
                'panel' => '#FEEBC8',
            ],
            default => [
                'background' => '#F4F7FB',
                'surface' => '#FFFFFF',
                'border' => '#D9E2EC',
                'accent' => '#1F5F8B',
                'panel' => '#EAF2F8',
            ],
        };
    }

    protected function truncateText(string $value, int $limit): string
    {
        $normalized = trim($value);

        if (mb_strlen($normalized) <= $limit) {
            return $normalized;
        }

        return rtrim(mb_substr($normalized, 0, $limit - 1)) . '...';
    }

    protected function escapeSvgText(string $value): string
    {
        return htmlspecialchars($value, ENT_QUOTES | ENT_XML1, 'UTF-8');
    }
}
