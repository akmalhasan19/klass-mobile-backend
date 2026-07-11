<?php

namespace App\Services;

use App\Models\RecommendedProject;
use App\Models\SystemRecommendationAssignment;
use App\Models\User;
use Carbon\CarbonImmutable;
use Carbon\CarbonInterface;
use Illuminate\Support\Collection;

class SystemRecommendationAssignmentService
{
    /**
     * @param  Collection<int, array<string, mixed>>  $items
     */
    public function trackServedRecommendations(
        User $user,
        Collection $items,
        ?CarbonInterface $moment = null,
    ): void {
        $moment = $moment ? CarbonImmutable::instance($moment) : CarbonImmutable::now();

        $rows = $items
            ->map(fn (array $item): ?array => $this->buildAssignmentRow($user, $item, $moment))
            ->filter()
            ->unique(fn (array $row): string => $row['recommendation_key'])
            ->values();

        if ($rows->isEmpty()) {
            return;
        }

        SystemRecommendationAssignment::query()->upsert(
            $rows->all(),
            ['user_id', 'recommendation_key'],
            [
                'recommendation_item_id',
                'source_type',
                'source_reference',
                'subject_id',
                'sub_subject_id',
                'last_distributed_at',
                'updated_at',
            ],
        );
    }

    /**
     * @param  array<string, mixed>  $item
     * @return array<string, mixed>|null
     */
    protected function buildAssignmentRow(User $user, array $item, CarbonImmutable $moment): ?array
    {
        $sourceType = $this->normalizeString(data_get($item, 'source_type'));

        if ($sourceType === null || ! $this->isTrackableSourceType($sourceType)) {
            return null;
        }

        $recommendationItemId = $this->normalizeString(data_get($item, 'id'));
        $sourceReference = $this->resolveSourceReference($item, $sourceType);
        $recommendationKey = $this->makeRecommendationKey($sourceType, $sourceReference);

        if ($recommendationItemId === null || $recommendationKey === null || $sourceReference === null) {
            return null;
        }

        return [
            'user_id' => $user->id,
            'recommendation_key' => $recommendationKey,
            'recommendation_item_id' => $recommendationItemId,
            'source_type' => $sourceType,
            'source_reference' => $sourceReference,
            'subject_id' => $this->normalizeNullableInt(data_get($item, 'subject_id')),
            'sub_subject_id' => $this->normalizeNullableInt(data_get($item, 'sub_subject_id')),
            'first_distributed_at' => $moment,
            'last_distributed_at' => $moment,
            'created_at' => $moment,
            'updated_at' => $moment,
        ];
    }

    protected function isTrackableSourceType(string $sourceType): bool
    {
        return in_array($sourceType, $this->trackableSourceTypes(), true);
    }

    /**
     * @return array<int, string>
     */
    protected function trackableSourceTypes(): array
    {
        $configuredSourceTypes = config('personalized_project_recommendations.distribution_summary.eligible_source_types', [
            RecommendedProject::SOURCE_SYSTEM_TOPIC,
            RecommendedProject::SOURCE_AI_GENERATED,
        ]);

        return collect($configuredSourceTypes)
            ->map(fn (mixed $value): string => (string) $value)
            ->filter(fn (string $value): bool => $value !== '')
            ->unique()
            ->values()
            ->all();
    }

    /**
     * @param  array<string, mixed>  $item
     */
    protected function resolveSourceReference(array $item, string $sourceType): ?string
    {
        $candidates = [
            data_get($item, 'source_reference'),
            data_get($item, 'source_payload.source_reference'),
        ];

        if ($sourceType === RecommendedProject::SOURCE_SYSTEM_TOPIC) {
            $candidates[] = data_get($item, 'source_payload.topic_id');
            $candidates[] = $this->extractTopicReferenceFromItemId($this->normalizeString(data_get($item, 'id')));
        }

        $candidates[] = data_get($item, 'id');

        foreach ($candidates as $candidate) {
            $normalized = $this->normalizeString($candidate);

            if ($normalized !== null) {
                return $normalized;
            }
        }

        return null;
    }

    protected function makeRecommendationKey(string $sourceType, ?string $sourceReference): ?string
    {
        if ($sourceType === '' || $sourceReference === null) {
            return null;
        }

        return $sourceType . ':' . $sourceReference;
    }

    protected function extractTopicReferenceFromItemId(?string $itemId): ?string
    {
        if ($itemId === null || ! str_starts_with($itemId, 'system_topic_')) {
            return null;
        }

        $topicId = substr($itemId, strlen('system_topic_'));

        return $topicId !== '' ? $topicId : null;
    }

    protected function normalizeNullableInt(mixed $value): ?int
    {
        return is_numeric($value) ? (int) $value : null;
    }

    protected function normalizeString(mixed $value): ?string
    {
        if ($value === null) {
            return null;
        }

        $normalized = trim((string) $value);

        return $normalized !== '' ? $normalized : null;
    }
}