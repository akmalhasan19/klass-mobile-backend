<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\StoreTopicRequest;
use App\Http\Requests\UpdateTopicRequest;
use App\Http\Resources\TopicResource;
use App\Http\Traits\ApiResponseTrait;
use App\Models\Topic;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class TopicController extends Controller
{
    use ApiResponseTrait;

    /**
     * Menampilkan daftar topics (dengan contents).
     *
     * GET /api/topics
     *   ?search=keyword   — Cari berdasarkan title
     *   ?per_page=15      — Jumlah item per halaman (max 50)
     */
    public function index(Request $request): JsonResponse
    {
        $includeContents = $request->boolean('include_contents');

        $query = Topic::query()
            ->select([
                'id',
                'title',
                'teacher_id',
                'sub_subject_id',
                'owner_user_id',
                'ownership_status',
                'thumbnail_url',
                'is_published',
                'order',
                'created_at',
                'updated_at',
            ])
            ->with('subSubject.subject');

        if ($includeContents) {
            $query->with([
                'contents' => fn ($contentsQuery) => $contentsQuery
                    ->select([
                        'id',
                        'topic_id',
                        'type',
                        'title',
                        'data',
                        'media_url',
                        'is_published',
                        'order',
                        'created_at',
                        'updated_at',
                    ])
                    ->orderBy('order')
                    ->orderBy('created_at'),
            ]);
        } else {
            $query->withCount('contents');
        }

        $likeOperator = DB::connection()->getDriverName() === 'pgsql' ? 'ilike' : 'like';

        // Search by title
        if ($search = $request->query('search')) {
            $query->where('title', $likeOperator, "%{$search}%");
        }

        // Filter by teacher_id
        if ($teacherId = $request->query('teacher_id')) {
            $query->where('teacher_id', $teacherId);
        }

        $perPage = min((int) $request->query('per_page', 15), 50);
        $paginator = $query
            ->orderBy('order')
            ->latest()
            ->paginate($perPage);

        return $this->paginated($paginator, TopicResource::class);
    }

    /**
     * Menyimpan topic baru.
     */
    public function store(StoreTopicRequest $request): JsonResponse
    {
        $attributes = $request->topicAttributes();
        $user = $request->user();

        if ($user?->isTeacher()) {
            $attributes['teacher_id'] = (string) $user->id;
        }

        $topic = Topic::create($attributes);
        $topic->loadMissing('subSubject.subject');

        return $this->created(
            new TopicResource($topic),
            'Topik berhasil dibuat.',
        );
    }

    /**
     * Menampilkan detail satu topic.
     */
    public function show(Topic $topic): JsonResponse
    {
        $topic->load(['contents.tasks', 'subSubject.subject']);

        return $this->success(
            new TopicResource($topic),
            'Detail topik berhasil diambil.',
        );
    }

    /**
     * Mengupdate topic.
     */
    public function update(UpdateTopicRequest $request, Topic $topic): JsonResponse
    {
        $topic->fill($request->topicAttributes());
        $topic->save();
        $topic->loadMissing('subSubject.subject');

        return $this->success(
            new TopicResource($topic),
            'Topik berhasil diupdate.',
        );
    }

    /**
     * Menghapus topic.
     */
    public function destroy(Topic $topic): JsonResponse
    {
        $topic->delete();

        return $this->noContent('Topik berhasil dihapus.');
    }
}
