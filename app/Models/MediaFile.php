<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class MediaFile extends Model
{
    use HasUuids;

    protected $fillable = [
        'uploader_id',
        'file_path',
        'file_name',
        'mime_type',
        'size',
        'disk',
        'category',
    ];

    /**
     * Get the user who uploaded the media.
     */
    public function uploader(): BelongsTo
    {
        return $this->belongsTo(User::class, 'uploader_id');
    }
    /**
     * Get the public URL of the media file.
     */
    public function getUrlAttribute(): string
    {
        $supabaseProjectUrl = env('SUPABASE_URL');
        $bucket = config('filesystems.disks.supabase.bucket', 'klass-storage');

        if ($supabaseProjectUrl) {
            $supabaseProjectUrl = rtrim($supabaseProjectUrl, '/');
            return "{$supabaseProjectUrl}/storage/v1/object/public/{$bucket}/{$this->file_path}";
        }

        // Fallback for non-Supabase storage (e.g. local public disk)
        if ($this->disk === 'public') {
            return \Illuminate\Support\Facades\Storage::disk('public')->url($this->file_path);
        }

        // Final fallback for dev or other disks
        $endpoint = rtrim(config("filesystems.disks.{$this->disk}.endpoint", ''), '/');
        return "{$endpoint}/{$bucket}/{$this->file_path}";
    }
}
