<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\StoreMarketplaceTaskRequest;
use App\Http\Requests\UpdateMarketplaceTaskRequest;
use App\Http\Resources\MarketplaceTaskResource;
use App\Http\Traits\ApiResponseTrait;
use App\Models\MarketplaceTask;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class MarketplaceTaskController extends Controller
{
    use ApiResponseTrait;

    /**
     * Menampilkan daftar marketplace tasks.
     *
     * GET /api/marketplace-tasks
     *   ?search=keyword            — Cari (melalui relasi content.title)
     *   ?status=open|taken|done    — Filter berdasarkan status
     *   ?content_id=uuid           — Filter berdasarkan konten
     *   ?per_page=15               — Jumlah item per halaman (max 50)
     */
    public function index(Request $request): JsonResponse
    {
        $query = MarketplaceTask::with('content');
        $likeOperator = DB::connection()->getDriverName() === 'pgsql' ? 'ilike' : 'like';

        // Search via content title
        if ($search = $request->query('search')) {
            $query->whereHas('content', function ($q) use ($search, $likeOperator) {
                $q->where('title', $likeOperator, "%{$search}%");
            });
        }

        // Filter by status
        if ($status = $request->query('status')) {
            $query->where('status', $status);
        }

        // Filter by content_id
        if ($contentId = $request->query('content_id')) {
            $query->where('content_id', $contentId);
        }

        $perPage = min((int) $request->query('per_page', 15), 50);
        $paginator = $query->latest()->paginate($perPage);

        return $this->paginated($paginator, MarketplaceTaskResource::class);
    }

    /**
     * Menyimpan marketplace task baru.
     */
    public function store(StoreMarketplaceTaskRequest $request): JsonResponse
    {
        $task = MarketplaceTask::create($request->validated());
        $task->load('content');

        return $this->created(
            new MarketplaceTaskResource($task),
            'Task berhasil dibuat.',
        );
    }

    /**
     * Menampilkan detail satu marketplace task.
     */
    public function show(MarketplaceTask $marketplaceTask): JsonResponse
    {
        $marketplaceTask->load('content.topic');

        return $this->success(
            new MarketplaceTaskResource($marketplaceTask),
            'Detail task berhasil diambil.',
        );
    }

    /**
     * Mengupdate marketplace task.
     */
    public function update(UpdateMarketplaceTaskRequest $request, MarketplaceTask $marketplaceTask): JsonResponse
    {
        $marketplaceTask->update($request->validated());

        return $this->success(
            new MarketplaceTaskResource($marketplaceTask),
            'Task berhasil diupdate.',
        );
    }

    /**
     * Menghapus marketplace task.
     */
    public function destroy(MarketplaceTask $marketplaceTask): JsonResponse
    {
        $marketplaceTask->delete();

        return $this->noContent('Task berhasil dihapus.');
    }
}
