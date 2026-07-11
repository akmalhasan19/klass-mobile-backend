<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Content extends Model
{
    use HasFactory, HasUuids;

    protected $fillable = [
        'topic_id',
        'type',
        'title',
        'data',
        'media_url',
        'is_published',
        'order',
    ];

    protected function casts(): array
    {
        return [
            'data' => 'array',
        ];
    }

    /**
     * Content milik satu Topic.
     */
    public function topic(): BelongsTo
    {
        return $this->belongsTo(Topic::class);
    }

    /**
     * Satu Content bisa punya banyak MarketplaceTask.
     */
    public function tasks(): HasMany
    {
        return $this->hasMany(MarketplaceTask::class);
    }
}
