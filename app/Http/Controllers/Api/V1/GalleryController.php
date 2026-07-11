<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Resources\ContentResource;
use App\Http\Traits\ApiResponseTrait;
use App\Models\Content;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

/**
 * GalleryController
 *
 * Endpoint untuk menampilkan galeri material/aset yang memiliki media.
 * Data berasal dari tabel `contents` yang punya `media_url`.
 *
 * GET /api/gallery
 *   ?search=keyword          — Cari berdasarkan title
 *   ?type=module|quiz|brief  — Filter berdasarkan tipe content
 *   ?topic_id=uuid           — Filter berdasarkan topik
 *   ?per_page=15             — Jumlah item per halaman
 */
class GalleryController extends Controller
{
    use ApiResponseTrait;

    public function index(Request $request): JsonResponse
    {
        $query = Content::with('topic')
            ->whereNotNull('media_url')
            ->where('media_url', '!=', '');
        $likeOperator = DB::connection()->getDriverName() === 'pgsql' ? 'ilike' : 'like';

        // Search by title
        if ($search = $request->query('search')) {
            $query->where('title', $likeOperator, "%{$search}%");
        }

        // Filter by type
        if ($type = $request->query('type')) {
            $query->where('type', $type);
        }

        // Filter by topic_id
        if ($topicId = $request->query('topic_id')) {
            $query->where('topic_id', $topicId);
        }

        $perPage = min((int) $request->query('per_page', 15), 50);
        $paginator = $query->latest()->paginate($perPage);

        return $this->paginated($paginator, ContentResource::class);
    }
}
