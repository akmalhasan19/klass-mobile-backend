<?php

namespace App\Services;

use App\Models\SubSubject;
use App\Models\Subject;
use App\Models\Topic;
use App\Models\User;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\DB;

class RecommendationPersonalizationService
{
    /**
     * @return array{public: array<string, mixed>, internal: array<string, mixed>}
     */
    public function resolve(?User $user): array
    {
        if (! $user) {
            return $this->guestContext();
        }

        $user->loadMissing('primarySubject');

        $primarySubject = $user->primarySubject;
        $authoredActivity = $this->resolveAuthoredTopicActivity($user);
        $hasAuthoredActivity = $authoredActivity->isNotEmpty();
        $signalsAvailable = $primarySubject !== null || $hasAuthoredActivity;

        $preferredActivity = $primarySubject
            ? $authoredActivity
                ->filter(fn (array $row) => (int) $row['subject_id'] === (int) $primarySubject->id)
                ->values()
            : $authoredActivity->values();

        $secondaryActivity = $primarySubject
            ? $authoredActivity
                ->reject(fn (array $row) => (int) $row['subject_id'] === (int) $primarySubject->id)
                ->values()
            : collect();

        $candidateSubSubjects = $preferredActivity
            ->concat($secondaryActivity)
            ->values();

        $subjectAnchor = $primarySubject
            ? $this->serializeSubject($primarySubject, 'profile')
            : $this->withSource(
                $candidateSubSubjects->first()['subject'] ?? null,
                $hasAuthoredActivity ? 'authored_topic_activity' : null,
            );

        return [
            'public' => [
                'signals_available' => $signalsAvailable,
                'has_primary_subject' => $primarySubject !== null,
                'has_authored_topic_activity' => $hasAuthoredActivity,
                'signal_source' => $this->resolveSignalSource($primarySubject, $hasAuthoredActivity),
                'fallback_mode' => $this->resolveFallbackMode($primarySubject, $preferredActivity, $hasAuthoredActivity),
                'primary_subject' => $this->serializeSubject($primarySubject),
                'subject_anchor' => $subjectAnchor,
                'candidate_sub_subject_ids' => $candidateSubSubjects
                    ->pluck('sub_subject_id')
                    ->map(fn (mixed $id) => (int) $id)
                    ->values()
                    ->all(),
                'candidate_sub_subjects' => $candidateSubSubjects
                    ->map(fn (array $row) => $this->serializeActivityRow($row))
                    ->values()
                    ->all(),
            ],
            'internal' => [
                'signals_available' => $signalsAvailable,
                'primary_subject_id' => $primarySubject?->id,
                'preferred_activity_sub_subject_ids' => $preferredActivity
                    ->pluck('sub_subject_id')
                    ->map(fn (mixed $id) => (int) $id)
                    ->values()
                    ->all(),
                'secondary_activity_sub_subject_ids' => $secondaryActivity
                    ->pluck('sub_subject_id')
                    ->map(fn (mixed $id) => (int) $id)
                    ->values()
                    ->all(),
                'personalized_mode' => 'personalized_system_candidate_selection',
                'personalized_description' => $this->resolvePersonalizedDescription($primarySubject, $hasAuthoredActivity),
            ],
        ];
    }

    /**
     * @return Collection<int, array<string, mixed>>
     */
    protected function resolveAuthoredTopicActivity(User $user): Collection
    {
        $activityRows = Topic::query()
            ->eligibleForPersonalization()
            ->where('owner_user_id', $user->id)
            ->join('sub_subjects', 'sub_subjects.id', '=', 'topics.sub_subject_id')
            ->select([
                'topics.sub_subject_id',
                'sub_subjects.subject_id',
                DB::raw('COUNT(*) as topic_count'),
                DB::raw('MAX(COALESCE(topics.updated_at, topics.created_at)) as latest_topic_activity_at'),
            ])
            ->groupBy('topics.sub_subject_id', 'sub_subjects.subject_id')
            ->orderByDesc('topic_count')
            ->orderByDesc('latest_topic_activity_at')
            ->orderBy('topics.sub_subject_id')
            ->get();

        if ($activityRows->isEmpty()) {
            return collect();
        }

        $subSubjects = SubSubject::query()
            ->with('subject')
            ->whereIn('id', $activityRows->pluck('sub_subject_id')->all())
            ->get()
            ->keyBy('id');

        return $activityRows
            ->map(function (object $row) use ($subSubjects): array {
                $subSubject = $subSubjects->get((int) $row->sub_subject_id);
                $subject = $subSubject?->subject;

                return [
                    'sub_subject_id' => (int) $row->sub_subject_id,
                    'subject_id' => (int) $row->subject_id,
                    'topic_count' => (int) $row->topic_count,
                    'latest_topic_activity_at' => $row->latest_topic_activity_at,
                    'sub_subject' => $this->serializeSubSubject($subSubject),
                    'subject' => $this->serializeSubject($subject),
                ];
            })
            ->values();
    }

    /**
     * @param  array<string, mixed>  $row
     * @return array<string, mixed>
     */
    protected function serializeActivityRow(array $row): array
    {
        return [
            'sub_subject_id' => $row['sub_subject_id'],
            'subject_id' => $row['subject_id'],
            'topic_count' => $row['topic_count'],
            'latest_topic_activity_at' => $row['latest_topic_activity_at'],
            'subject' => $row['subject'],
            'sub_subject' => $row['sub_subject'],
            'source' => 'authored_topic_activity',
        ];
    }

    /**
     * @return array{public: array<string, mixed>, internal: array<string, mixed>}
     */
    protected function guestContext(): array
    {
        return [
            'public' => [
                'signals_available' => false,
                'has_primary_subject' => false,
                'has_authored_topic_activity' => false,
                'signal_source' => 'guest',
                'fallback_mode' => 'global_feed',
                'primary_subject' => null,
                'subject_anchor' => null,
                'candidate_sub_subject_ids' => [],
                'candidate_sub_subjects' => [],
            ],
            'internal' => [
                'signals_available' => false,
                'primary_subject_id' => null,
                'preferred_activity_sub_subject_ids' => [],
                'secondary_activity_sub_subject_ids' => [],
                'personalized_mode' => 'personalized_system_candidate_selection',
                'personalized_description' => 'Select and order system-generated recommendations using authenticated user personalization signals.',
            ],
        ];
    }

    protected function resolveSignalSource(?Subject $primarySubject, bool $hasAuthoredActivity): string
    {
        if ($primarySubject !== null && $hasAuthoredActivity) {
            return 'profile_subject_with_authored_activity';
        }

        if ($primarySubject !== null) {
            return 'profile_subject';
        }

        if ($hasAuthoredActivity) {
            return 'authored_topic_activity';
        }

        return 'insufficient_signals';
    }

    protected function resolveFallbackMode(?Subject $primarySubject, Collection $preferredActivity, bool $hasAuthoredActivity): ?string
    {
        if ($primarySubject === null && ! $hasAuthoredActivity) {
            return 'global_feed';
        }

        if ($primarySubject !== null && $preferredActivity->isEmpty()) {
            return 'primary_subject_catalog';
        }

        return null;
    }

    protected function resolvePersonalizedDescription(?Subject $primarySubject, bool $hasAuthoredActivity): string
    {
        if ($primarySubject !== null && $hasAuthoredActivity) {
            return 'Select and order system-generated recommendations using the user primary subject and authored-topic activity.';
        }

        if ($primarySubject !== null) {
            return 'Select and order system-generated recommendations using the user primary subject when authored-topic activity is still sparse.';
        }

        return 'Select and order system-generated recommendations using authored-topic activity because no primary subject is set.';
    }

    /**
     * @return array<string, mixed>|null
     */
    protected function serializeSubject(?Subject $subject, ?string $source = null): ?array
    {
        if (! $subject) {
            return null;
        }

        return $this->withSource([
            'id' => $subject->id,
            'name' => $subject->name,
            'slug' => $subject->slug,
        ], $source);
    }

    /**
     * @return array<string, mixed>|null
     */
    protected function serializeSubSubject(?SubSubject $subSubject): ?array
    {
        if (! $subSubject) {
            return null;
        }

        return [
            'id' => $subSubject->id,
            'subject_id' => $subSubject->subject_id,
            'name' => $subSubject->name,
            'slug' => $subSubject->slug,
        ];
    }

    /**
     * @param  array<string, mixed>|null  $payload
     * @return array<string, mixed>|null
     */
    protected function withSource(?array $payload, ?string $source): ?array
    {
        if ($payload === null) {
            return null;
        }

        if ($source !== null) {
            $payload['source'] = $source;
        }

        return $payload;
    }
}