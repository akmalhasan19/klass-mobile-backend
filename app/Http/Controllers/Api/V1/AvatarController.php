<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\StoreAvatarRequest;
use App\Http\Resources\UserResource;
use App\Http\Traits\ApiResponseTrait;
use App\Services\FileUploadService;
use Illuminate\Http\JsonResponse;

/**
 * AvatarController
 *
 * Endpoint khusus upload avatar user.
 * Upload ke Supabase Storage kategori 'avatars',
 * kemudian update `users.avatar_url`.
 *
 * POST /api/user/avatar
 */
class AvatarController extends Controller
{
    use ApiResponseTrait;

    public function __construct(
        protected FileUploadService $uploadService,
    ) {}

    /**
     * Upload avatar dan update profil user.
     */
    public function store(StoreAvatarRequest $request): JsonResponse
    {
        try {
            $result = $this->uploadService->upload(
                $request->file('file'),
                'avatars',
            );

            /** @var \App\Models\User $user */
            $user = $request->user();
            $user->update(['avatar_url' => $result['url']]);

            return $this->success([
                'user' => new UserResource($user->fresh()),
                'avatar_url' => $result['url'],
            ], 'Avatar berhasil diupload.');

        } catch (\Throwable $e) {
            report($e);

            return $this->error(
                'Gagal mengupload avatar. Silakan coba lagi.',
                500,
            );
        }
    }
}
