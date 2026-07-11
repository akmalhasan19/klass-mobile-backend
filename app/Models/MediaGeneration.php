<?php

namespace App\Models;

use App\MediaGeneration\MediaGenerationLifecycle;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Support\Str;

class MediaGeneration extends Model
{
    use HasFactory, HasUuids;

    protected $fillable = [
        'teacher_id',
        'generated_from_id',
        'is_regeneration',
        'subject_id',
        'sub_subject_id',
        'topic_id',
        'content_id',
        'recommended_project_id',
        'raw_prompt',
        'request_fingerprint',
        'active_duplicate_key',
        'preferred_output_type',
        'resolved_output_type',
        'status',
        'llm_provider',
        'llm_model',
        'generator_provider',
        'generator_model',
        'interpretation_payload',
        'interpretation_audit_payload',
        'generation_spec_payload',
        'decision_payload',
        'orchestration_audit_payload',
        'delivery_payload',
        'generator_service_response',
        'storage_path',
        'file_url',
        'thumbnail_url',
        'mime_type',
        'error_code',
        'error_message',
    ];

    protected function casts(): array
    {
        return [
            'teacher_id' => 'integer',
            'subject_id' => 'integer',
            'sub_subject_id' => 'integer',
            'is_regeneration' => 'boolean',
            'interpretation_payload' => 'array',
            'interpretation_audit_payload' => 'array',
            'generation_spec_payload' => 'array',
            'decision_payload' => 'array',
            'orchestration_audit_payload' => 'array',
            'delivery_payload' => 'array',
            'generator_service_response' => 'array',
        ];
    }

    protected static function booted(): void
    {
        static::saving(function (MediaGeneration $generation): void {
            $generation->preferred_output_type = self::normalizePreferredOutputType($generation->preferred_output_type);

            if ($generation->teacher_id !== null && trim((string) $generation->raw_prompt) !== '') {
                $generation->request_fingerprint = self::makeRequestFingerprint(
                    teacherId: (int) $generation->teacher_id,
                    rawPrompt: (string) $generation->raw_prompt,
                    preferredOutputType: $generation->preferred_output_type,
                    subjectId: $generation->subject_id,
                    subSubjectId: $generation->sub_subject_id,
                );
            }

            $generation->active_duplicate_key = $generation->shouldPreventDuplicateSubmission()
                ? $generation->request_fingerprint
                : null;
        });
    }

    public function teacher(): BelongsTo
    {
        return $this->belongsTo(User::class, 'teacher_id');
    }

    /**
     * The parent generation this was regenerated from (if any).
     */
    public function parentGeneration(): BelongsTo
    {
        return $this->belongsTo(self::class, 'generated_from_id');
    }

    /**
     * All child generations that were regenerated from this one.
     */
    public function childGenerations(): HasMany
    {
        return $this->hasMany(self::class, 'generated_from_id');
    }

    public function subject(): BelongsTo
    {
        return $this->belongsTo(Subject::class);
    }

    public function subSubject(): BelongsTo
    {
        return $this->belongsTo(SubSubject::class);
    }

    public function topic(): BelongsTo
    {
        return $this->belongsTo(Topic::class);
    }

    public function content(): BelongsTo
    {
        return $this->belongsTo(Content::class);
    }

    public function recommendedProject(): BelongsTo
    {
        return $this->belongsTo(RecommendedProject::class);
    }

    /**
     * FreelancerMatch records computed for this generation.
     */
    public function freelancerMatches(): HasMany
    {
        return $this->hasMany(FreelancerMatch::class);
    }

    public function scopeForTeacher(Builder $query, int $teacherId): Builder
    {
        return $query->where('teacher_id', $teacherId);
    }

    public function scopeRecentFirst(Builder $query): Builder
    {
        return $query
            ->orderByDesc('created_at')
            ->orderByDesc('id');
    }

    public function scopeActiveDuplicates(Builder $query, int $teacherId, string $requestFingerprint): Builder
    {
        return $query
            ->forTeacher($teacherId)
            ->where('request_fingerprint', $requestFingerprint)
            ->whereNotIn('status', MediaGenerationLifecycle::terminalStates());
    }

    public function jobKey(): string
    {
        return 'media-generation:' . $this->id;
    }

    public function isTerminal(): bool
    {
        return MediaGenerationLifecycle::isTerminal($this->status);
    }

    /**
     * Check whether this generation is a regeneration of a previous one.
     */
    public function isRegeneration(): bool
    {
        return (bool) $this->is_regeneration;
    }

    /**
     * Walk the parent chain upward to find the original root generation.
     *
     * Returns $this if this generation has no parent (i.e. it is the root).
     */
    public function getOriginalGeneration(): self
    {
        $current = $this;

        // Safety limit to prevent infinite loops in case of data corruption
        $maxDepth = 50;
        $depth = 0;

        while ($current->generated_from_id !== null && $depth < $maxDepth) {
            $current = $current->parentGeneration;

            if ($current === null) {
                // Parent was deleted — return the last known generation
                return $this;
            }

            $depth++;
        }

        return $current;
    }

    public function shouldPreventDuplicateSubmission(): bool
    {
        return trim((string) $this->request_fingerprint) !== ''
            && ! in_array($this->status, MediaGenerationLifecycle::terminalStates(), true);
    }

    public static function normalizePreferredOutputType(?string $preferredOutputType): string
    {
        if ($preferredOutputType === null || trim($preferredOutputType) === '') {
            return 'auto';
        }

        $normalized = strtolower(trim($preferredOutputType));

        if (! in_array($normalized, MediaPromptInterpretationSchema::allowedPreferredOutputTypes(), true)) {
            return 'auto';
        }

        return $normalized;
    }

    public static function makeRequestFingerprint(
        int $teacherId,
        string $rawPrompt,
        ?string $preferredOutputType = null,
        ?int $subjectId = null,
        ?int $subSubjectId = null,
    ): string {
        $normalizedPrompt = Str::of($rawPrompt)
            ->squish()
            ->lower()
            ->toString();

        return hash('sha256', implode('|', [
            $teacherId,
            self::normalizePreferredOutputType($preferredOutputType),
            $subjectId ?? 'none',
            $subSubjectId ?? 'none',
            $normalizedPrompt,
        ]));
    }
}