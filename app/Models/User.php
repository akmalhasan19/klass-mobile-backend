<?php

namespace App\Models;

// use Illuminate\Contracts\Auth\MustVerifyEmail;
use Database\Factories\UserFactory;
use Illuminate\Database\Eloquent\Attributes\Fillable;
use Illuminate\Database\Eloquent\Attributes\Hidden;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Foundation\Auth\User as Authenticatable;
use Illuminate\Notifications\Notifiable;
use Illuminate\Support\Facades\DB;
use Laravel\Sanctum\HasApiTokens;

#[Fillable(['name', 'email', 'password', 'avatar_url', 'primary_subject_id', 'role', 'security_question', 'security_answer'])]
#[Hidden(['password', 'remember_token', 'security_answer'])]
class User extends Authenticatable
{
    /** @use HasFactory<UserFactory> */
    use HasApiTokens, HasFactory, Notifiable;

    public const ROLE_ADMIN = 'admin';
    public const ROLE_USER = 'user'; // Legacy — treated as teacher for backward compat
    public const ROLE_TEACHER = 'teacher';
    public const ROLE_FREELANCER = 'freelancer';

    public function isAdmin(): bool
    {
        return $this->role === self::ROLE_ADMIN;
    }

    /**
     * A user is considered a teacher if their role is explicitly 'teacher'
     * or the legacy 'user' role (backward compatibility).
     */
    public function isTeacher(): bool
    {
        return in_array($this->role, [self::ROLE_TEACHER, self::ROLE_USER], true);
    }

    public function isFreelancer(): bool
    {
        return $this->role === self::ROLE_FREELANCER;
    }

    public function authoredTopics(): HasMany
    {
        return $this->hasMany(Topic::class, 'owner_user_id');
    }

    public function mediaGenerations(): HasMany
    {
        return $this->hasMany(MediaGeneration::class, 'teacher_id');
    }

    public function systemRecommendationAssignments(): HasMany
    {
        return $this->hasMany(SystemRecommendationAssignment::class);
    }

    public function primarySubject(): BelongsTo
    {
        return $this->belongsTo(Subject::class, 'primary_subject_id');
    }

    public function hasPrimarySubjectProfile(): bool
    {
        return $this->primary_subject_id !== null;
    }

    public function resolvePersonalizationSubjectAnchor(): ?Subject
    {
        return $this->resolveStoredPrimarySubject()
            ?? $this->resolveAuthoredTopicFallbackSubject();
    }

    public function resolvePersonalizationSubjectSource(): ?string
    {
        if ($this->resolveStoredPrimarySubject() !== null) {
            return 'profile';
        }

        return $this->resolveAuthoredTopicFallbackSubject() !== null
            ? 'authored_topic_activity'
            : null;
    }

    public function resolveAuthoredTopicFallbackSubject(): ?Subject
    {
        $candidate = $this->authoredTopics()
            ->eligibleForPersonalization()
            ->join('sub_subjects', 'sub_subjects.id', '=', 'topics.sub_subject_id')
            ->select([
                'sub_subjects.subject_id',
                DB::raw('COUNT(*) as topic_count'),
                DB::raw('MAX(topics.updated_at) as latest_topic_activity_at'),
            ])
            ->groupBy('sub_subjects.subject_id')
            ->orderByDesc('topic_count')
            ->orderByDesc('latest_topic_activity_at')
            ->orderBy('sub_subjects.subject_id')
            ->first();

        return $candidate?->subject_id
            ? Subject::query()->find($candidate->subject_id)
            : null;
    }

    /**
     * Get the attributes that should be cast.
     *
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'email_verified_at' => 'datetime',
            'primary_subject_id' => 'integer',
            'password' => 'hashed',
        ];
    }

    protected function resolveStoredPrimarySubject(): ?Subject
    {
        if ($this->primary_subject_id === null) {
            return null;
        }

        if ($this->relationLoaded('primarySubject')) {
            return $this->getRelation('primarySubject');
        }

        return $this->primarySubject()->first();
    }
}
