<?php

namespace App\Models;

use Carbon\CarbonInterface;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Support\Carbon;

class RecommendedProject extends Model
{
    use HasFactory;

    public const SOURCE_ADMIN_UPLOAD = 'admin_upload';
    public const SOURCE_SYSTEM_TOPIC = 'system_topic';
    public const SOURCE_AI_GENERATED = 'ai_generated';

    protected $fillable = [
        'title',
        'description',
        'thumbnail_url',
        'project_file_url',
        'ratio',
        'project_type',
        'tags',
        'modules',
        'source_type',
        'source_reference',
        'source_payload',
        'display_priority',
        'is_active',
        'starts_at',
        'ends_at',
        'created_by',
        'updated_by',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'tags' => 'array',
            'modules' => 'array',
            'source_payload' => 'array',
            'display_priority' => 'integer',
            'is_active' => 'boolean',
            'starts_at' => 'datetime',
            'ends_at' => 'datetime',
        ];
    }

    public function creator(): BelongsTo
    {
        return $this->belongsTo(User::class, 'created_by');
    }

    public function updater(): BelongsTo
    {
        return $this->belongsTo(User::class, 'updated_by');
    }

    public function scopeVisibleAt(Builder $query, CarbonInterface|string|null $moment = null): Builder
    {
        $moment = $moment instanceof CarbonInterface
            ? Carbon::instance($moment)
            : ($moment ? Carbon::parse($moment) : now());

        return $query
            ->where('is_active', true)
            ->where(function (Builder $builder) use ($moment) {
                $builder->whereNull('starts_at')
                    ->orWhere('starts_at', '<=', $moment);
            })
            ->where(function (Builder $builder) use ($moment) {
                $builder->whereNull('ends_at')
                    ->orWhere('ends_at', '>=', $moment);
            });
    }
}