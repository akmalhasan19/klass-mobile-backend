<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class MarketplaceTask extends Model
{
    use HasFactory, HasUuids;

    /** Task type constants */
    public const TYPE_BID = 'bid';
    public const TYPE_SUGGESTION = 'suggestion';

    /** Status constants */
    public const STATUS_OPEN = 'open';
    public const STATUS_OPEN_FOR_BID = 'open_for_bid';
    public const STATUS_ASSIGNED = 'assigned';
    public const STATUS_TAKEN = 'taken';
    public const STATUS_DONE = 'done';

    protected $fillable = [
        'content_id',
        'media_generation_id',
        'status',
        'task_type',
        'description',
        'creator_id',
        'suggested_freelancer_id',
        'attachment_url',
    ];

    /**
     * MarketplaceTask milik satu Content.
     */
    public function content(): BelongsTo
    {
        return $this->belongsTo(Content::class);
    }

    /**
     * The MediaGeneration that triggered this refinement task.
     */
    public function mediaGeneration(): BelongsTo
    {
        return $this->belongsTo(MediaGeneration::class);
    }

    /**
     * The freelancer auto-suggested for this task (if task_type = 'suggestion').
     */
    public function suggestedFreelancer(): BelongsTo
    {
        return $this->belongsTo(User::class, 'suggested_freelancer_id');
    }

    /**
     * The teacher/user who created this task.
     */
    public function creator(): BelongsTo
    {
        return $this->belongsTo(User::class, 'creator_id');
    }

    /**
     * Freelancer bids on this task (for open bid tasks).
     * This relationship targets a future FreelancerBid model
     * and is a placeholder that will be implemented when bidding is built.
     */
    public function freelancerBids(): HasMany
    {
        // Placeholder — will point to FreelancerBid model once bidding is implemented
        return $this->hasMany(MarketplaceTask::class, 'id', 'id')->whereRaw('1 = 0');
    }

    /**
     * Check if this is a suggestion-type task (auto-matched).
     */
    public function isSuggestionTask(): bool
    {
        return $this->task_type === self::TYPE_SUGGESTION;
    }

    /**
     * Check if this is a bid-type task (open posting).
     */
    public function isBidTask(): bool
    {
        return $this->task_type === self::TYPE_BID;
    }
}
