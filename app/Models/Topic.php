<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Topic extends Model
{
    use HasFactory, HasUuids;

    public const OWNERSHIP_STATUS_NORMALIZED = 'normalized';
    public const OWNERSHIP_STATUS_LEGACY_UNRESOLVED = 'legacy_unresolved';
    public const PERSONALIZATION_MODE_CANDIDATE = 'candidate';
    public const PERSONALIZATION_MODE_GENERAL_FEED_ONLY = 'general_feed_only';
    public const PERSONALIZATION_EXCLUSION_MISSING_SUB_SUBJECT = 'missing_sub_subject';
    public const PERSONALIZATION_EXCLUSION_UNRESOLVED_OWNERSHIP = 'unresolved_ownership';

    protected $fillable = [
        'title',
        'teacher_id',
        'sub_subject_id',
        'owner_user_id',
        'ownership_status',
        'thumbnail_url',
        'is_published',
        'order',
    ];

    protected function casts(): array
    {
        return [
            'sub_subject_id' => 'integer',
            'owner_user_id' => 'integer',
            'is_published' => 'boolean',
            'order' => 'integer',
        ];
    }

    protected static function booted(): void
    {
        static::saving(function (Topic $topic): void {
            $topic->syncOwnershipFromLegacyIdentifier();
        });
    }

    /**
     * Satu Topic memiliki banyak Content.
     */
    public function contents(): HasMany
    {
        return $this->hasMany(Content::class);
    }

    public function owner(): BelongsTo
    {
        return $this->belongsTo(User::class, 'owner_user_id');
    }

    public function subSubject(): BelongsTo
    {
        return $this->belongsTo(SubSubject::class);
    }

    public function scopeNormalizedOwnership(Builder $query): Builder
    {
        return $query
            ->whereNotNull('owner_user_id')
            ->where('ownership_status', self::OWNERSHIP_STATUS_NORMALIZED);
    }

    public function scopeEligibleForPersonalization(Builder $query): Builder
    {
        return $query
            ->normalizedOwnership()
            ->whereNotNull('sub_subject_id');
    }

    public function syncOwnershipFromLegacyIdentifier(): void
    {
        if (! $this->isDirty('owner_user_id')) {
            $this->owner_user_id = $this->resolveOwnerUserIdFromTeacherIdentifier();
        }

        $this->ownership_status = $this->owner_user_id !== null
            ? self::OWNERSHIP_STATUS_NORMALIZED
            : self::OWNERSHIP_STATUS_LEGACY_UNRESOLVED;
    }

    public function hasNormalizedOwnership(): bool
    {
        return $this->owner_user_id !== null
            && $this->ownership_status === self::OWNERSHIP_STATUS_NORMALIZED;
    }

    public function hasAdequateTaxonomy(): bool
    {
        return $this->sub_subject_id !== null;
    }

    public function isEligibleForPersonalization(): bool
    {
        return $this->hasAdequateTaxonomy()
            && $this->hasNormalizedOwnership();
    }

    public function resolvePersonalizationMode(): string
    {
        return $this->isEligibleForPersonalization()
            ? self::PERSONALIZATION_MODE_CANDIDATE
            : self::PERSONALIZATION_MODE_GENERAL_FEED_ONLY;
    }

    public function resolvePersonalizationExclusionReason(): ?string
    {
        if (! $this->hasAdequateTaxonomy()) {
            return self::PERSONALIZATION_EXCLUSION_MISSING_SUB_SUBJECT;
        }

        if (! $this->hasNormalizedOwnership()) {
            return self::PERSONALIZATION_EXCLUSION_UNRESOLVED_OWNERSHIP;
        }

        return null;
    }

    /**
     * @return array<string, mixed>
     */
    public function resolvePersonalizationContext(): array
    {
        return [
            'eligible' => $this->isEligibleForPersonalization(),
            'mode' => $this->resolvePersonalizationMode(),
            'has_adequate_taxonomy' => $this->hasAdequateTaxonomy(),
            'has_normalized_ownership' => $this->hasNormalizedOwnership(),
            'excluded_reason' => $this->resolvePersonalizationExclusionReason(),
        ];
    }

    public function resolveSubject(): ?Subject
    {
        return $this->subSubject?->subject;
    }

    protected function resolveOwnerUserIdFromTeacherIdentifier(): ?int
    {
        $teacherIdentifier = trim((string) $this->teacher_id);

        if ($teacherIdentifier === '') {
            return null;
        }

        if (preg_match('/^\d+$/', $teacherIdentifier) === 1) {
            return User::query()->whereKey((int) $teacherIdentifier)->value('id');
        }

        if (filter_var($teacherIdentifier, FILTER_VALIDATE_EMAIL)) {
            return User::query()
                ->whereRaw('LOWER(email) = ?', [strtolower($teacherIdentifier)])
                ->value('id');
        }

        return null;
    }
}
