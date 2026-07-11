<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\StoreStudentProgressRequest;
use App\Http\Requests\UpdateStudentProgressRequest;
use App\Http\Resources\StudentProgressResource;
use App\Http\Traits\ApiResponseTrait;
use App\Models\StudentProgress;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class StudentProgressController extends Controller
{
    use ApiResponseTrait;

    /**
     * Menampilkan daftar student progress.
     *
     * GET /api/student-progress
     *   ?search=keyword   — Cari berdasarkan student_name
     *   ?per_page=15      — Jumlah item per halaman (max 50)
     */
    public function index(Request $request): JsonResponse
    {
        $query = StudentProgress::query();
        $likeOperator = DB::connection()->getDriverName() === 'pgsql' ? 'ilike' : 'like';

        // Search by student name
        if ($search = $request->query('search')) {
            $query->where('student_name', $likeOperator, "%{$search}%");
        }

        $perPage = min((int) $request->query('per_page', 15), 50);
        $paginator = $query->orderByDesc('completion_date')->paginate($perPage);

        return $this->paginated($paginator, StudentProgressResource::class);
    }

    /**
     * Menyimpan student progress baru.
     */
    public function store(StoreStudentProgressRequest $request): JsonResponse
    {
        $progress = StudentProgress::create($request->validated());

        return $this->created(
            new StudentProgressResource($progress),
            'Progress siswa berhasil dibuat.',
        );
    }

    /**
     * Menampilkan detail satu student progress.
     */
    public function show(StudentProgress $studentProgress): JsonResponse
    {
        return $this->success(
            new StudentProgressResource($studentProgress),
            'Detail progress siswa berhasil diambil.',
        );
    }

    /**
     * Mengupdate student progress.
     */
    public function update(UpdateStudentProgressRequest $request, StudentProgress $studentProgress): JsonResponse
    {
        $studentProgress->update($request->validated());

        return $this->success(
            new StudentProgressResource($studentProgress),
            'Progress siswa berhasil diupdate.',
        );
    }

    /**
     * Menghapus student progress.
     */
    public function destroy(StudentProgress $studentProgress): JsonResponse
    {
        $studentProgress->delete();

        return $this->noContent('Progress siswa berhasil dihapus.');
    }
}
