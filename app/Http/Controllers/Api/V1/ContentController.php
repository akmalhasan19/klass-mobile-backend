<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\StoreContentRequest;
use App\Http\Requests\UpdateContentRequest;
use App\Http\Resources\ContentResource;
use App\Http\Traits\ApiResponseTrait;
use App\Models\Content;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class ContentController extends Controller
{
    use ApiResponseTrait;

    /**
     * Menampilkan daftar contents.
     *
     * GET /api/contents
     *   ?search=keyword            — Cari berdasarkan title
     *   ?topic_id=uuid             — Filter berdasarkan topik
     *   ?type=module|quiz|brief    — Filter berdasarkan tipe
     *   ?per_page=15               — Jumlah item per halaman (max 50)
     */
    public function index(Request $request): JsonResponse
    {
        $query = Content::with('topic');
        $likeOperator = DB::connection()->getDriverName() === 'pgsql' ? 'ilike' : 'like';

        // Search by title
        if ($search = $request->query('search')) {
            $query->where('title', $likeOperator, "%{$search}%");
        }

        // Filter by topic_id
        if ($topicId = $request->query('topic_id')) {
            $query->where('topic_id', $topicId);
        }

        // Filter by type
        if ($type = $request->query('type')) {
            $query->where('type', $type);
        }

        $perPage = min((int) $request->query('per_page', 15), 50);
        $paginator = $query->latest()->paginate($perPage);

        return $this->paginated($paginator, ContentResource::class);
    }

    /**
     * Menyimpan content baru.
     */
    public function store(StoreContentRequest $request): JsonResponse
    {
        $content = Content::create($request->validated());
        $content->load('topic');

        return $this->created(
            new ContentResource($content),
            'Konten berhasil dibuat.',
        );
    }

    /**
     * Menampilkan detail satu content.
     */
    public function show(Content $content): JsonResponse
    {
        $content->load(['topic', 'tasks']);

        return $this->success(
            new ContentResource($content),
            'Detail konten berhasil diambil.',
        );
    }

    /**
     * Mengupdate content.
     */
    public function update(UpdateContentRequest $request, Content $content): JsonResponse
    {
        $content->update($request->validated());

        return $this->success(
            new ContentResource($content),
            'Konten berhasil diupdate.',
        );
    }

    /**
     * Menghapus content.
     */
    public function destroy(Content $content): JsonResponse
    {
        $content->delete();

        return $this->noContent('Konten berhasil dihapus.');
    }
}
