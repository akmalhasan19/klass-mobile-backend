<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Represents a computed match between a freelancer and a media generation.
 *
 * Created during the auto-suggest hiring flow to record how well each
 * freelancer candidate scored against a given generation's requirements.
 * Serves as an audit trail and enables retry/re-rank logic.
 *
 * @property int $id
 * @property string $media_generation_id
 * @property int $freelancer_id
 * @property float $match_score
 * @property float $portfolio_relevance_score
 * @property float $success_rate
 */
class FreelancerMatch extends Model
{
    use HasFactory;

    protected $fillable = [
        'media_generation_id',
        'freelancer_id',
        'match_score',
        'portfolio_relevance_score',
        'success_rate',
    ];

    protected function casts(): array
    {
        return [
            'freelancer_id' => 'integer',
            'match_score' => 'float',
            'portfolio_relevance_score' => 'float',
            'success_rate' => 'float',
        ];
    }

    /**
     * The media generation this match was computed for.
     */
    public function mediaGeneration(): BelongsTo
    {
        return $this->belongsTo(MediaGeneration::class);
    }

    /**
     * The freelancer candidate.
     */
    public function freelancer(): BelongsTo
    {
        return $this->belongsTo(User::class, 'freelancer_id');
    }
}
