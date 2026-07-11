<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\FileUploadRequest;
use App\Http\Traits\ApiResponseTrait;
use App\Services\FileUploadService;
use Illuminate\Http\JsonResponse;

/**
 * FileUploadController
 *
 * Endpoint untuk upload file ke Supabase Storage bucket.
 * Mendukung kategori: avatars, gallery, materials, attachments.
 *
 * POST   /api/upload/{category}  — Upload file
 * DELETE /api/upload/{category}  — Hapus file (query param: path)
 */
class FileUploadController extends Controller
{
    use ApiResponseTrait;

    public function __construct(
        protected FileUploadService $uploadService,
    ) {}

    /**
     * Upload file ke kategori yang ditentukan.
     */
    public function upload(FileUploadRequest $request, string $category): JsonResponse
    {
        try {
            $result = $this->uploadService->upload(
                $request->file('file'),
                $category,
            );

            return $this->created([
                'path' => $result['path'],
                'url' => $result['url'],
                'category' => $category,
            ], 'File berhasil di-upload.');

        } catch (\InvalidArgumentException $e) {
            return $this->error($e->getMessage(), 422);

        } catch (\Illuminate\Validation\ValidationException $e) {
            return $this->validationError($e->errors(), 'Validasi gagal.');

        } catch (\Throwable $e) {
            report($e);

            return $this->error('Gagal meng-upload file. Silakan coba lagi.', 500);
        }
    }

    /**
     * Hapus file dari bucket.
     *
     * DELETE /api/upload/{category}?path=avatars/1234_abc_photo.jpg
     */
    public function destroy(string $category, FileUploadService $uploadService): JsonResponse
    {
        $path = request()->query('path');

        if (!$path) {
            return $this->error('Parameter "path" wajib dikirim.', 422);
        }

        try {
            $deleted = $uploadService->delete($path);

            if ($deleted) {
                return $this->success(null, 'File berhasil dihapus.');
            }

            return $this->notFound('File tidak ditemukan.');

        } catch (\Throwable $e) {
            report($e);

            return $this->error('Gagal menghapus file. Silakan coba lagi.', 500);
        }
    }
}
