<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class SystemRecommendationAssignment extends Model
{
    use HasFactory;

    protected $fillable = [
        'user_id',
        'recommendation_key',
        'recommendation_item_id',
        'source_type',
        'source_reference',
        'subject_id',
        'sub_subject_id',
        'first_distributed_at',
        'last_distributed_at',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'user_id' => 'integer',
            'subject_id' => 'integer',
            'sub_subject_id' => 'integer',
            'first_distributed_at' => 'datetime',
            'last_distributed_at' => 'datetime',
        ];
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    public function subject(): BelongsTo
    {
        return $this->belongsTo(Subject::class);
    }

    public function subSubject(): BelongsTo
    {
        return $this->belongsTo(SubSubject::class);
    }
}