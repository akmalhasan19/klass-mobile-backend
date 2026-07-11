<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use App\Models\RecommendedProject;
use App\Services\FileUploadService;
use App\Services\ThumbnailGeneratorService;
use Illuminate\Http\Request;
use Illuminate\Http\UploadedFile;

class AdminRecommendedProjectController extends Controller
{
    protected FileUploadService $fileUploadService;
    protected ThumbnailGeneratorService $thumbnailGeneratorService;

    public function __construct(FileUploadService $fileUploadService, ThumbnailGeneratorService $thumbnailGeneratorService)
    {
        $this->fileUploadService = $fileUploadService;
        $this->thumbnailGeneratorService = $thumbnailGeneratorService;
    }

    public function store(Request $request)
    {
        $validated = $request->validate([
            'title' => 'required|string|max:255',
            'description' => 'nullable|string',
            'ratio' => 'required|string',
            'project_type' => 'nullable|string',
            'tags' => 'nullable|string',
            'modules' => 'nullable|string',
            'thumbnail' => 'nullable|image|max:5120',
            'project_file' => 'nullable|file|mimes:pdf,ppt,pptx,doc,docx|max:10240',
            'display_priority' => 'nullable|integer',
            'is_active' => 'boolean',
            'starts_at' => 'nullable|date',
            'ends_at' => 'nullable|date|after_or_equal:starts_at',
        ]);

        try {
            $thumbnailUrl = null;
            if ($request->hasFile('thumbnail')) {
                $upload = $this->fileUploadService->upload($request->file('thumbnail'), 'gallery');
                $thumbnailUrl = $upload['url'];
            }

            $projectFileUrl = null;
            if ($request->hasFile('project_file')) {
                $upload = $this->fileUploadService->upload($request->file('project_file'), 'materials');
                $projectFileUrl = $upload['url'];
            }

            // Auto-generate thumbnail dari project_file jika tidak ada thumbnail
            if ($thumbnailUrl === null && $request->hasFile('project_file')) {
                $thumbnailUrl = $this->autoGenerateThumbnail($request->file('project_file'));
            }

            $project = RecommendedProject::create([
                'title' => $validated['title'],
                'description' => $validated['description'] ?? null,
                'ratio' => $validated['ratio'] ?? '16:9',
                'project_type' => $validated['project_type'] ?? null,
                'tags' => !empty($validated['tags']) ? array_map('trim', explode(',', $validated['tags'])) : null,
                'modules' => !empty($validated['modules']) ? array_map('trim', explode(',', $validated['modules'])) : null,
                'thumbnail_url' => $thumbnailUrl,
                'project_file_url' => $projectFileUrl,
                'source_type' => RecommendedProject::SOURCE_ADMIN_UPLOAD,
                'display_priority' => $validated['display_priority'] ?? 0,
                'is_active' => $request->has('is_active'),
                'starts_at' => $validated['starts_at'] ?? null,
                'ends_at' => $validated['ends_at'] ?? null,
                'created_by' => auth()->id(),
                'updated_by' => auth()->id(),
            ]);

            ActivityLog::create([
                'actor_id' => auth()->id(),
                'action' => 'create_recommended_project',
                'subject_type' => RecommendedProject::class,
                'subject_id' => $project->id,
                'metadata' => ['title' => $project->title],
            ]);

            return back()->with('success', 'Recommended Project created successfully.');
        } catch (\Exception $e) {
            return back()->withInput()->withErrors(['error' => 'Gagal mengupload file atau menyimpan data: ' . $e->getMessage()]);
        }
    }

    public function update(Request $request, RecommendedProject $recommendedProject)
    {
        $validated = $request->validate([
            'title' => 'required|string|max:255',
            'description' => 'nullable|string',
            'ratio' => 'required|string',
            'project_type' => 'nullable|string',
            'tags' => 'nullable|string',
            'modules' => 'nullable|string',
            'thumbnail' => 'nullable|image|max:5120',
            'project_file' => 'nullable|file|mimes:pdf,ppt,pptx,doc,docx|max:10240',
            'display_priority' => 'nullable|integer',
            'is_active' => 'boolean',
            'starts_at' => 'nullable|date',
            'ends_at' => 'nullable|date|after_or_equal:starts_at',
        ]);

        try {
            $thumbnailUrl = $recommendedProject->thumbnail_url;
            if ($request->hasFile('thumbnail')) {
                $upload = $this->fileUploadService->upload($request->file('thumbnail'), 'gallery');
                $thumbnailUrl = $upload['url'];
            }

            $projectFileUrl = $recommendedProject->project_file_url;
            if ($request->hasFile('project_file')) {
                $upload = $this->fileUploadService->upload($request->file('project_file'), 'materials');
                $projectFileUrl = $upload['url'];
            }

            // Auto-generate thumbnail jika:
            // 1. Tidak ada thumbnail saat ini DAN tidak ada thumbnail baru yang diupload
            // 2. Ada project_file baru yang diupload
            if ($thumbnailUrl === null && $request->hasFile('project_file')) {
                $thumbnailUrl = $this->autoGenerateThumbnail($request->file('project_file'));
            }

            $recommendedProject->update([
                'title' => $validated['title'],
                'description' => $validated['description'] ?? null,
                'ratio' => $validated['ratio'],
                'project_type' => $validated['project_type'] ?? null,
                'tags' => !empty($validated['tags']) ? array_map('trim', explode(',', $validated['tags'])) : null,
                'modules' => !empty($validated['modules']) ? array_map('trim', explode(',', $validated['modules'])) : null,
                'thumbnail_url' => $thumbnailUrl,
                'project_file_url' => $projectFileUrl,
                'display_priority' => $validated['display_priority'] ?? 0,
                'is_active' => $request->has('is_active'),
                'starts_at' => $validated['starts_at'] ?? null,
                'ends_at' => $validated['ends_at'] ?? null,
                'updated_by' => auth()->id(),
            ]);

            ActivityLog::create([
                'actor_id' => auth()->id(),
                'action' => 'update_recommended_project',
                'subject_type' => RecommendedProject::class,
                'subject_id' => $recommendedProject->id,
                'metadata' => ['title' => $recommendedProject->title],
            ]);

            return back()->with('success', 'Recommended Project updated successfully.');
        } catch (\Exception $e) {
            return back()->withInput()->withErrors(['error' => 'Gagal mengupload file atau menyimpan data: ' . $e->getMessage()]);
        }
    }

    public function destroy(RecommendedProject $recommendedProject)
    {
        $title = $recommendedProject->title;
        $recommendedProject->delete();

        ActivityLog::create([
            'actor_id' => auth()->id(),
            'action' => 'delete_recommended_project',
            'subject_type' => RecommendedProject::class,
            'subject_id' => $recommendedProject->id,
            'metadata' => ['title' => $title],
        ]);

        return back()->with('success', 'Recommended Project deleted successfully.');
    }
    
    public function toggleActive(RecommendedProject $recommendedProject)
    {
        $recommendedProject->update(['is_active' => !$recommendedProject->is_active]);

        ActivityLog::create([
            'actor_id' => auth()->id(),
            'action' => 'toggle_active_recommended_project',
            'subject_type' => RecommendedProject::class,
            'subject_id' => $recommendedProject->id,
            'metadata' => ['is_active' => $recommendedProject->is_active],
        ]);

        return back()->with('success', 'Project status toggled successfully.');
    }

    public function showNow(RecommendedProject $recommendedProject)
    {
        $recommendedProject->update([
            'is_active' => true,
            'starts_at' => null,
        ]);

        ActivityLog::create([
            'actor_id' => auth()->id(),
            'action' => 'show_now_recommended_project',
            'subject_type' => RecommendedProject::class,
            'subject_id' => $recommendedProject->id,
            'metadata' => ['title' => $recommendedProject->title],
        ]);

        return back()->with('success', 'Project is successfully set to show now.');
    }

    /**
     * Auto-generate thumbnail dari UploadedFile (project_file).
     * Menggunakan ThumbnailGeneratorService untuk extract halaman/slide pertama,
     * lalu upload hasilnya ke Supabase Storage via FileUploadService.
     *
     * @return string|null  URL thumbnail yang di-generate, atau null jika gagal
     */
    protected function autoGenerateThumbnail(UploadedFile $projectFile): ?string
    {
        try {
            $tempThumbnailPath = $this->thumbnailGeneratorService->generateFromFile(
                $projectFile->getRealPath()
            );

            if ($tempThumbnailPath === null) {
                return null;
            }

            // Upload thumbnail yang di-generate ke Supabase Storage  
            $generatedFile = new UploadedFile(
                $tempThumbnailPath,
                'auto_thumbnail_' . pathinfo($projectFile->getClientOriginalName(), PATHINFO_FILENAME) . '.' . pathinfo($tempThumbnailPath, PATHINFO_EXTENSION),
                mime_content_type($tempThumbnailPath),
                null,
                true // test mode agar tidak perlu is_uploaded_file check
            );

            $upload = $this->fileUploadService->upload($generatedFile, 'gallery');

            // Cleanup temp file
            @unlink($tempThumbnailPath);

            return $upload['url'];
        } catch (\Throwable $e) {
            report($e);
            return null;
        }
    }
}
