<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use App\Models\MediaFile;
use App\Services\FileUploadService;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Storage;
use Illuminate\View\View;

class AdminMediaController extends Controller
{
    protected FileUploadService $fileUploadService;

    public function __construct(FileUploadService $fileUploadService)
    {
        $this->fileUploadService = $fileUploadService;
    }

    public function index(Request $request): View
    {
        $category = $request->query('category');
        $search = $request->query('search');
        $type = $request->query('type'); // New: filter by mime type prefix (image, video, etc)

        $medias = MediaFile::query()
            ->with('uploader')
            ->when($category, fn($q) => $q->where('category', $category))
            ->when($type, fn($q) => $q->where('mime_type', 'like', "{$type}/%"))
            ->when($search, fn($q) => $q->where('file_name', 'like', "%{$search}%"))
            ->latest()
            ->paginate(15)
            ->withQueryString();

        // Get unique categories for filter
        $categories = MediaFile::select('category')->distinct()->pluck('category');

        return view('admin.media.index', compact('medias', 'category', 'search', 'categories', 'type'));
    }

    /**
     * Handle file upload.
     */
    public function store(Request $request)
    {
        $request->validate([
            'files' => 'required|array',
            'files.*' => 'required|file',
            'category' => 'required|string',
        ]);

        $category = $request->input('category');
        $uploadedCount = 0;

        foreach ($request->file('files') as $file) {
            try {
                $result = $this->fileUploadService->upload($file, $category);

                MediaFile::create([
                    'uploader_id' => auth()->id(),
                    'file_path'   => $result['path'],
                    'file_name'   => $file->getClientOriginalName(),
                    'mime_type'   => $file->getMimeType(),
                    'size'        => $file->getSize(),
                    'disk'        => 'supabase',
                    'category'    => $category,
                ]);

                $uploadedCount++;
            } catch (\Exception $e) {
                // Log or handle individual file failure
                continue;
            }
        }

        return back()->with('success', "{$uploadedCount} file berhasil diunggah.");
    }

    /**
     * Delete a single media file.
     */
    public function destroy(MediaFile $media)
    {
        $disk = $media->disk ?? 'public';
        $path = $media->file_path;
        $id = $media->id;

        // Perform physical deletion
        Storage::disk($disk)->delete($path);

        // DB record deletion
        $media->delete();

        ActivityLog::create([
            'actor_id'     => auth()->id(),
            'action'       => 'delete_media',
            'subject_type' => MediaFile::class,
            'subject_id'   => $id,
            'metadata'     => [
                'disk' => $disk,
                'path' => $path,
            ],
        ]);

        return back()->with('success', 'Media berhasil dihapus.');
    }

    /**
     * Bulk delete media files.
     */
    public function bulkDestroy(Request $request)
    {
        $ids = $request->input('ids', []);
        
        if (empty($ids)) {
            return back()->with('error', 'Tidak ada file yang dipilih.');
        }

        $medias = MediaFile::whereIn('id', $ids)->get();
        $deletedCount = 0;

        foreach ($medias as $media) {
            $disk = $media->disk ?? 'public';
            $path = $media->file_path;

            Storage::disk($disk)->delete($path);
            $media->delete();
            $deletedCount++;
        }

        return back()->with('success', "{$deletedCount} file berhasil dihapus secara massal.");
    }
}
