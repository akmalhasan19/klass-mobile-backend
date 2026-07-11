<?php

namespace App\Services;

use App\Models\RecommendedProject;
use App\Models\SubSubject;
use App\Models\SystemRecommendationAssignment;
use App\Models\Topic;
use Carbon\CarbonImmutable;
use Carbon\CarbonInterface;
use Illuminate\Support\Collection;
use Throwable;

class RecommendationAggregationService
{
    /**
     * @return Collection<int, array<string, mixed>>
     */
    public function buildFeed(?CarbonInterface $moment = null, ?array $personalizationContext = null): Collection
    {
        return $this->buildFeedSnapshot($moment, $personalizationContext)['items'];
    }

    /**
     * @return Collection<int, array<string, mixed>>
     */
    public function buildSystemDistributionSummary(?int $minimumDistinctUserCount = null): Collection
    {
        $candidates = $this->getSystemDistributionSummaryCandidates($minimumDistinctUserCount);

        if ($candidates->isEmpty()) {
            return collect();
        }

        $maximumItemsPerSubSubject = max(
            (int) config('personalized_project_recommendations.distribution_summary.maximum_items_per_sub_subject', 1),
            1,
        );

        return $candidates
            ->groupBy(fn (array $item) => (int) data_get($item, 'sub_subject_id', 0))
            ->map(fn (Collection $items) => $items
                ->sort(fn (array $left, array $right) => $this->compareSystemDistributionCandidates($left, $right))
                ->take($maximumItemsPerSubSubject)
                ->values())
            ->collapse()
            ->sort(fn (array $left, array $right) => ((int) data_get($left, 'subject.id', data_get($left, 'subject_id', 0))
                    <=> (int) data_get($right, 'subject.id', data_get($right, 'subject_id', 0)))
                ?: ((int) data_get($left, 'sub_subject.id', data_get($left, 'sub_subject_id', 0))
                    <=> (int) data_get($right, 'sub_subject.id', data_get($right, 'sub_subject_id', 0)))
                ?: $this->compareSystemDistributionCandidates($left, $right))
            ->values();
    }

    /**
     * @return array{items: array<int, array<string, mixed>>, empty_state: array<string, mixed>}
     */
    public function buildAdminSystemDistributionSummaryPayload(?int $minimumDistinctUserCount = null): array
    {
        $items = $this->buildSystemDistributionSummary($minimumDistinctUserCount)
            ->map(fn (array $item): array => $this->normalizeAdminSystemDistributionSummaryItem($item))
            ->values()
            ->all();

        return [
            'items' => $items,
            'empty_state' => [
                'is_empty' => $items === [],
                'message' => (string) config(
                    'personalized_project_recommendations.homepage.admin_sections.system_distribution_empty_state',
                    'No system recommendation has been distributed to more than one user yet.',
                ),
            ],
        ];
    }

    /**
     * @return array{items: Collection<int, array<string, mixed>>, source_status: array<string, array<string, mixed>>, personalization: array<string, mixed>}
     */
    public function buildFeedSnapshot(?CarbonInterface $moment = null, ?array $personalizationContext = null): array
    {
        $moment = $moment ? CarbonImmutable::instance($moment) : CarbonImmutable::now();

        $curatedItems = $this->getVisibleCuratedItems($moment);
        $adminCuratedItems = $curatedItems
            ->filter(fn (array $item) => $this->isAdminCuratedItem($item))
            ->values();
        $persistedSystemGeneratedItems = $curatedItems
            ->reject(fn (array $item) => $this->isAdminCuratedItem($item))
            ->values();
        $suppressedSourceKeys = $this->getSuppressedNonAdminSourceKeys();
        $topicResult = $this->getNormalizedTopicItemsSafely($suppressedSourceKeys, $personalizationContext);
        $systemGeneratedResult = $this->selectSystemGeneratedCandidates(
            $persistedSystemGeneratedItems->concat($topicResult['items'])->values(),
            $personalizationContext,
        );

        $items = $adminCuratedItems
            ->concat($systemGeneratedResult['items'])
            ->sort(fn (array $left, array $right) => $this->compareItems($left, $right))
            ->values();

        return [
            'items' => $items,
            'source_status' => [
                RecommendedProject::SOURCE_ADMIN_UPLOAD => [
                    'state' => $this->resolveStateFromCount(
                        $curatedItems->where('source_type', RecommendedProject::SOURCE_ADMIN_UPLOAD)->count()
                    ),
                ],
                RecommendedProject::SOURCE_SYSTEM_TOPIC => [
                    'state' => $topicResult['state'],
                    'suppressed_count' => count($suppressedSourceKeys),
                ],
                RecommendedProject::SOURCE_AI_GENERATED => [
                    'state' => $this->resolveStateFromCount(
                        $persistedSystemGeneratedItems->where('source_type', RecommendedProject::SOURCE_AI_GENERATED)->count()
                    ),
                ],
            ],
            'personalization' => $systemGeneratedResult['summary'],
        ];
    }

    /**
     * @param  array<int, string>  $suppressedSourceKeys
     * @return array{items: Collection<int, array<string, mixed>>, state: string}
     */
    protected function getNormalizedTopicItemsSafely(array $suppressedSourceKeys, ?array $personalizationContext = null): array
    {
        try {
            $items = $this->getNormalizedTopicItems($suppressedSourceKeys);

            return [
                'items' => $items,
                'state' => $this->resolveStateFromCount($items->count()),
            ];
        } catch (Throwable $throwable) {
            report($throwable);

            return [
                'items' => collect(),
                'state' => 'failed',
            ];
        }
    }

    /**
     * @return Collection<int, array<string, mixed>>
     */
    protected function getVisibleCuratedItems(CarbonInterface $moment): Collection
    {
        $projects = RecommendedProject::query()
            ->visibleAt($moment)
            ->get();

        $sourceTopics = Topic::query()
            ->select([
                'id',
                'title',
                'teacher_id',
                'sub_subject_id',
                'owner_user_id',
                'ownership_status',
                'thumbnail_url',
                'is_published',
                'order',
                'created_at',
                'updated_at',
            ])
            ->with('subSubject.subject')
            ->whereIn(
                'id',
                $projects
                    ->where('source_type', RecommendedProject::SOURCE_SYSTEM_TOPIC)
                    ->pluck('source_reference')
                    ->filter()
                    ->unique()
                    ->values()
                    ->all(),
            )
            ->get()
            ->keyBy(fn (Topic $topic) => (string) $topic->id);

        return $projects->map(
            fn (RecommendedProject $project) => $this->normalizeRecommendedProject(
                $project,
                $sourceTopics->get((string) $project->source_reference),
            )
        );
    }

    /**
     * @return array<int, string>
     */
    protected function getSuppressedNonAdminSourceKeys(): array
    {
        return RecommendedProject::query()
            ->whereNotNull('source_reference')
            ->where('source_type', '!=', RecommendedProject::SOURCE_ADMIN_UPLOAD)
            ->get(['source_type', 'source_reference', 'source_payload'])
            ->flatMap(function (RecommendedProject $project): array {
                $keys = array_filter([
                    $this->makeSourceKey($project->source_type, $project->source_reference),
                ]);

                $generatedTopicId = data_get($project->source_payload, 'topic_id');

                if ($project->source_type === RecommendedProject::SOURCE_AI_GENERATED && $generatedTopicId) {
                    $keys[] = $this->makeSourceKey(RecommendedProject::SOURCE_SYSTEM_TOPIC, $generatedTopicId);
                }

                return array_values(array_filter($keys));
            })
            ->filter()
            ->unique()
            ->values()
            ->all();
    }

    /**
     * @param  array<int, string>  $suppressedSourceKeys
     * @return Collection<int, array<string, mixed>>
     */
    protected function getNormalizedTopicItems(array $suppressedSourceKeys): Collection
    {
        $suppressedLookup = array_fill_keys($suppressedSourceKeys, true);

        return Topic::query()
            ->select([
                'id',
                'title',
                'teacher_id',
                'sub_subject_id',
                'owner_user_id',
                'ownership_status',
                'thumbnail_url',
                'is_published',
                'order',
                'created_at',
                'updated_at',
            ])
            ->where('is_published', true)
            ->with([
                'contents' => fn ($query) => $query
                    ->select([
                        'id',
                        'topic_id',
                        'type',
                        'title',
                        'is_published',
                        'order',
                        'created_at',
                    ])
                    ->where('is_published', true)
                    ->orderBy('order')
                    ->orderByDesc('created_at'),
                'subSubject.subject',
            ])
            ->get()
            ->reject(function (Topic $topic) use ($suppressedLookup) {
                $sourceKey = $this->makeSourceKey(RecommendedProject::SOURCE_SYSTEM_TOPIC, $topic->id);

                return isset($suppressedLookup[$sourceKey]);
            })
            ->map(fn (Topic $topic) => $this->normalizeTopic($topic));
    }

    /**
     * @return array<string, mixed>
     */
    protected function normalizeRecommendedProject(RecommendedProject $project, ?Topic $sourceTopic = null): array
    {
        $sourcePayload = is_array($project->source_payload) ? $project->source_payload : [];
        $sourcePayload = $this->backfillSystemTopicSourcePayload($project, $sourcePayload, $sourceTopic);
        $sourceType = (string) $project->source_type;

        return [
            'id' => (string) $project->id,
            'title' => $project->title,
            'description' => $project->description,
            'thumbnail_url' => $project->thumbnail_url,
            'ratio' => $project->ratio ?: '16:9',
            'project_type' => $project->project_type,
            'tags' => $project->tags ?? [],
            'modules' => $project->modules ?? [],
            'sub_subject_id' => data_get($sourcePayload, 'sub_subject_id'),
            'subject_id' => data_get($sourcePayload, 'subject_id'),
            'taxonomy' => data_get($sourcePayload, 'taxonomy'),
            'personalization' => data_get($sourcePayload, 'personalization'),
            'source_type' => $sourceType,
            'source_reference' => $project->source_reference,
            'source_payload' => $sourcePayload,
            'feed_origin' => $sourceType === RecommendedProject::SOURCE_ADMIN_UPLOAD
                ? 'admin_curated'
                : 'system_generated',
            'display_priority' => (int) $project->display_priority,
            'score' => $this->extractScore($sourcePayload),
            'visibility' => [
                'is_active' => (bool) $project->is_active,
                'starts_at' => $project->starts_at,
                'ends_at' => $project->ends_at,
            ],
            'created_at' => $project->created_at,
            'updated_at' => $project->updated_at,
        ];
    }

    /**
     * @return array<string, mixed>
     */
    protected function normalizeTopic(Topic $topic): array
    {
        $subSubject = $topic->subSubject;
        $subject = $subSubject?->subject;
        $personalization = $topic->resolvePersonalizationContext();
        $modules = $topic->contents
            ->map(function ($content) {
                return $content->title ?: $content->type;
            })
            ->filter()
            ->values()
            ->all();

        $taxonomy = $this->serializeTopicTaxonomy($subject, $subSubject);
        $subjectId = $subject?->id ?? $subSubject?->subject_id;

        return [
            'id' => 'system_topic_' . $topic->id,
            'title' => $topic->title,
            'description' => null,
            'thumbnail_url' => $topic->thumbnail_url,
            'ratio' => '16:9',
            'project_type' => null,
            'tags' => [],
            'modules' => $modules,
            'sub_subject_id' => $topic->sub_subject_id,
            'subject_id' => $subjectId,
            'taxonomy' => $taxonomy,
            'personalization' => $personalization,
            'source_type' => RecommendedProject::SOURCE_SYSTEM_TOPIC,
            'source_reference' => $topic->id,
            'source_payload' => [
                'topic_id' => $topic->id,
                'teacher_id' => $topic->teacher_id,
                'sub_subject_id' => $topic->sub_subject_id,
                'subject_id' => $subjectId,
                'taxonomy' => $taxonomy,
                'personalization' => $personalization,
                'owner_user_id' => $topic->owner_user_id,
                'ownership_status' => $topic->ownership_status,
                'topic_order' => $topic->order,
                'contents_count' => count($modules),
            ],
            'feed_origin' => 'system_generated',
            'display_priority' => 0,
            'score' => 0.0,
            'visibility' => [
                'is_active' => true,
                'starts_at' => null,
                'ends_at' => null,
            ],
            'created_at' => $topic->created_at,
            'updated_at' => $topic->updated_at,
        ];
    }

    /**
     * @param  array<string, mixed>  $sourcePayload
     * @return array<string, mixed>
     */
    protected function backfillSystemTopicSourcePayload(
        RecommendedProject $project,
        array $sourcePayload,
        ?Topic $sourceTopic = null,
    ): array {
        if ($project->source_type !== RecommendedProject::SOURCE_SYSTEM_TOPIC || ! $sourceTopic) {
            return $sourcePayload;
        }

        $subSubject = $sourceTopic->subSubject;
        $subject = $subSubject?->subject;
        $taxonomy = $this->serializeTopicTaxonomy($subject, $subSubject);
        $personalization = $sourceTopic->resolvePersonalizationContext();

        $sourcePayload['topic_id'] = $sourcePayload['topic_id'] ?? $sourceTopic->id;
        $sourcePayload['teacher_id'] = $sourcePayload['teacher_id'] ?? $sourceTopic->teacher_id;
        $sourcePayload['sub_subject_id'] = $sourcePayload['sub_subject_id'] ?? $sourceTopic->sub_subject_id;
        $sourcePayload['subject_id'] = $sourcePayload['subject_id'] ?? ($subject?->id ?? $subSubject?->subject_id);
        $sourcePayload['taxonomy'] = $sourcePayload['taxonomy'] ?? $taxonomy;
        $sourcePayload['personalization'] = $sourcePayload['personalization'] ?? $personalization;
        $sourcePayload['owner_user_id'] = $sourcePayload['owner_user_id'] ?? $sourceTopic->owner_user_id;
        $sourcePayload['ownership_status'] = $sourcePayload['ownership_status'] ?? $sourceTopic->ownership_status;
        $sourcePayload['topic_order'] = $sourcePayload['topic_order'] ?? $sourceTopic->order;

        return $sourcePayload;
    }

    /**
     * @return array<string, mixed>|null
     */
    protected function serializeTopicTaxonomy(mixed $subject, mixed $subSubject): ?array
    {
        if (! $subSubject) {
            return null;
        }

        return [
            'subject' => $subject ? [
                'id' => $subject->id,
                'name' => $subject->name,
                'slug' => $subject->slug,
            ] : null,
            'sub_subject' => [
                'id' => $subSubject->id,
                'subject_id' => $subSubject->subject_id,
                'name' => $subSubject->name,
                'slug' => $subSubject->slug,
            ],
        ];
    }

    /**
     * @param  Collection<int, array<string, mixed>>  $topicItems
     * @return array{items: Collection<int, array<string, mixed>>, summary: array<string, mixed>}
     */
    protected function selectSystemGeneratedCandidates(Collection $systemGeneratedItems, ?array $personalizationContext = null): array
    {
        if ($systemGeneratedItems->isEmpty()) {
            return [
                'items' => $systemGeneratedItems->values(),
                'summary' => $this->emptyPersonalizationSummary(),
            ];
        }

        if (! data_get($personalizationContext, 'internal.signals_available', false)) {
            return [
                'items' => $systemGeneratedItems->values(),
                'summary' => $this->emptyPersonalizationSummary(),
            ];
        }

        $primarySubjectId = data_get($personalizationContext, 'internal.primary_subject_id');
        $preferredLookup = $this->buildRankLookup(
            (array) data_get($personalizationContext, 'internal.preferred_activity_sub_subject_ids', [])
        );
        $secondaryLookup = $this->buildRankLookup(
            (array) data_get($personalizationContext, 'internal.secondary_activity_sub_subject_ids', [])
        );
        $catalogLookup = $this->buildPrimarySubjectCatalogLookup(
            $systemGeneratedItems,
            $primarySubjectId !== null ? (int) $primarySubjectId : null,
            array_merge(array_keys($preferredLookup), array_keys($secondaryLookup)),
        );

        $annotatedItems = $systemGeneratedItems
            ->map(function (array $item) use ($preferredLookup, $secondaryLookup, $catalogLookup, $primarySubjectId): array {
                $subSubjectId = data_get($item, 'sub_subject_id');
                $subjectId = data_get($item, 'subject_id');
                $isEligible = $this->isEligibleSystemGeneratedCandidate($item);

                $selection = [
                    'eligible' => $isEligible,
                    'selected' => false,
                    'group' => $isEligible ? 3 : 4,
                    'rank' => PHP_INT_MAX,
                    'reason' => $isEligible ? 'global_feed_fallback' : 'normalization_required',
                ];

                if ($isEligible && $subSubjectId !== null && isset($preferredLookup[(int) $subSubjectId])) {
                    $selection = [
                        'eligible' => true,
                        'selected' => true,
                        'group' => 0,
                        'rank' => $preferredLookup[(int) $subSubjectId],
                        'reason' => 'authored_topic_activity',
                    ];
                } elseif (
                    $isEligible
                    && $primarySubjectId !== null
                    && $subjectId !== null
                    && (int) $subjectId === (int) $primarySubjectId
                    && $subSubjectId !== null
                    && isset($catalogLookup[(int) $subSubjectId])
                ) {
                    $selection = [
                        'eligible' => true,
                        'selected' => true,
                        'group' => 1,
                        'rank' => $catalogLookup[(int) $subSubjectId],
                        'reason' => 'primary_subject_catalog',
                    ];
                } elseif ($isEligible && $subSubjectId !== null && isset($secondaryLookup[(int) $subSubjectId])) {
                    $selection = [
                        'eligible' => true,
                        'selected' => true,
                        'group' => 2,
                        'rank' => $secondaryLookup[(int) $subSubjectId],
                        'reason' => 'secondary_authored_topic_activity',
                    ];
                }

                $item['candidate_selection'] = $selection;

                return $item;
            })
            ->values();

        $selectedItems = $annotatedItems
            ->filter(fn (array $item) => (bool) data_get($item, 'candidate_selection.selected', false))
            ->values();

        if ($selectedItems->isEmpty()) {
            return [
                'items' => $systemGeneratedItems->values(),
                'summary' => $this->emptyPersonalizationSummary(),
            ];
        }

        return [
            'items' => $selectedItems,
            'summary' => $this->buildPersonalizationSummary($annotatedItems, $selectedItems, $personalizationContext),
        ];
    }

    protected function isEligibleSystemGeneratedCandidate(array $item): bool
    {
        if ($this->isAdminCuratedItem($item)) {
            return false;
        }

        if (data_get($item, 'sub_subject_id') === null || data_get($item, 'subject_id') === null) {
            return false;
        }

        if (data_get($item, 'source_type') === RecommendedProject::SOURCE_SYSTEM_TOPIC) {
            if (data_get($item, 'personalization') !== null) {
                return (bool) data_get($item, 'personalization.eligible', false);
            }

            return true;
        }

        return true;
    }

    /**
     * @param  array<int, int>  $subSubjectIds
     * @return array<int, int>
     */
    protected function buildRankLookup(array $subSubjectIds): array
    {
        $lookup = [];

        foreach (array_values(array_unique(array_map('intval', $subSubjectIds))) as $index => $subSubjectId) {
            $lookup[$subSubjectId] = $index;
        }

        return $lookup;
    }

    /**
     * @param  Collection<int, array<string, mixed>>  $topicItems
     * @param  array<int, int>  $excludedSubSubjectIds
     * @return array<int, int>
     */
    protected function buildPrimarySubjectCatalogLookup(
        Collection $topicItems,
        ?int $primarySubjectId,
        array $excludedSubSubjectIds,
    ): array {
        if ($primarySubjectId === null) {
            return [];
        }

        $excludedLookup = array_fill_keys(array_map('intval', $excludedSubSubjectIds), true);

        return $topicItems
            ->filter(fn (array $item) => $this->isEligibleSystemGeneratedCandidate($item))
            ->filter(fn (array $item) => (int) data_get($item, 'subject_id', 0) === $primarySubjectId)
            ->reject(function (array $item) use ($excludedLookup): bool {
                $subSubjectId = data_get($item, 'sub_subject_id');

                return $subSubjectId !== null && isset($excludedLookup[(int) $subSubjectId]);
            })
            ->groupBy(fn (array $item) => (int) data_get($item, 'sub_subject_id'))
            ->map(function (Collection $items, int $subSubjectId): array {
                return [
                    'sub_subject_id' => $subSubjectId,
                    'topic_count' => $items->count(),
                    'latest_topic_updated_at' => $items
                        ->map(fn (array $item) => $this->timestampValue(data_get($item, 'updated_at')))
                        ->max() ?? 0,
                ];
            })
            ->sort(fn (array $left, array $right) => (($right['topic_count'] ?? 0) <=> ($left['topic_count'] ?? 0))
                ?: (($right['latest_topic_updated_at'] ?? 0) <=> ($left['latest_topic_updated_at'] ?? 0))
                ?: (($left['sub_subject_id'] ?? 0) <=> ($right['sub_subject_id'] ?? 0)))
            ->values()
            ->mapWithKeys(fn (array $row, int $index) => [(int) $row['sub_subject_id'] => $index])
            ->all();
    }

    /**
     * @param  Collection<int, array<string, mixed>>  $topicItems
     * @return array<string, mixed>
     */
    protected function buildPersonalizationSummary(
        Collection $allSystemGeneratedItems,
        Collection $selectedSystemGeneratedItems,
        ?array $personalizationContext = null,
    ): array
    {
        $summary = [
            'applied' => $selectedSystemGeneratedItems->isNotEmpty(),
            'filter_applied' => $selectedSystemGeneratedItems->isNotEmpty(),
            'selected_system_candidate_count' => $selectedSystemGeneratedItems->count(),
            'filtered_out_system_candidate_count' => max($allSystemGeneratedItems->count() - $selectedSystemGeneratedItems->count(), 0),
            'matched_system_topic_count' => $selectedSystemGeneratedItems
                ->where('source_type', RecommendedProject::SOURCE_SYSTEM_TOPIC)
                ->count(),
        ];

        if ($selectedSystemGeneratedItems->isNotEmpty()) {
            $orderedMatchedItems = $selectedSystemGeneratedItems
                ->sort(fn (array $left, array $right) => ((int) data_get($left, 'candidate_selection.group', 3) <=> (int) data_get($right, 'candidate_selection.group', 3))
                    ?: ((int) data_get($left, 'candidate_selection.rank', PHP_INT_MAX) <=> (int) data_get($right, 'candidate_selection.rank', PHP_INT_MAX))
                    ?: ($this->timestampValue(data_get($right, 'updated_at')) <=> $this->timestampValue(data_get($left, 'updated_at'))))
                ->values();

            $summary['mode'] = (string) data_get(
                $personalizationContext,
                'internal.personalized_mode',
                'personalized_system_candidate_selection',
            );
            $summary['description'] = (string) data_get(
                $personalizationContext,
                'internal.personalized_description',
                'Select and order system-generated recommendations using authenticated user personalization signals.',
            );
            $summary['selected_source_breakdown'] = [
                RecommendedProject::SOURCE_SYSTEM_TOPIC => $selectedSystemGeneratedItems
                    ->where('source_type', RecommendedProject::SOURCE_SYSTEM_TOPIC)
                    ->count(),
                RecommendedProject::SOURCE_AI_GENERATED => $selectedSystemGeneratedItems
                    ->where('source_type', RecommendedProject::SOURCE_AI_GENERATED)
                    ->count(),
            ];
            $summary['matched_sub_subject_ids'] = $orderedMatchedItems
                ->pluck('sub_subject_id')
                ->filter()
                ->map(fn (mixed $id) => (int) $id)
                ->unique()
                ->values()
                ->all();
        }

        return $summary;
    }

    /**
     * @return array<string, mixed>
     */
    protected function emptyPersonalizationSummary(): array
    {
        return [
            'applied' => false,
            'filter_applied' => false,
            'matched_system_topic_count' => 0,
        ];
    }

    protected function compareItems(array $left, array $right): int
    {
        return $this->compareSystemGeneratedCandidateSelection($left, $right)
            ?: ((int) data_get($right, 'display_priority', 0) <=> (int) data_get($left, 'display_priority', 0))
            ?: ((float) data_get($right, 'score', 0) <=> (float) data_get($left, 'score', 0))
            ?: ($this->timestampValue(data_get($right, 'created_at')) <=> $this->timestampValue(data_get($left, 'created_at')))
            ?: strcmp((string) data_get($left, 'id'), (string) data_get($right, 'id'));
    }

    protected function compareSystemGeneratedCandidateSelection(array $left, array $right): int
    {
        if (! $this->isSystemGeneratedItem($left) || ! $this->isSystemGeneratedItem($right)) {
            return 0;
        }

        return ((int) data_get($left, 'candidate_selection.group', 3) <=> (int) data_get($right, 'candidate_selection.group', 3))
            ?: ((int) data_get($left, 'candidate_selection.rank', PHP_INT_MAX) <=> (int) data_get($right, 'candidate_selection.rank', PHP_INT_MAX))
            ?: ($this->timestampValue(data_get($right, 'updated_at')) <=> $this->timestampValue(data_get($left, 'updated_at')));
    }

    protected function isAdminCuratedItem(array $item): bool
    {
        return data_get($item, 'source_type') === RecommendedProject::SOURCE_ADMIN_UPLOAD;
    }

    protected function isSystemGeneratedItem(array $item): bool
    {
        return ! $this->isAdminCuratedItem($item);
    }

    protected function makeSourceKey(?string $sourceType, mixed $sourceReference): ?string
    {
        if (! $sourceType || $sourceReference === null || $sourceReference === '') {
            return null;
        }

        return $sourceType . ':' . $sourceReference;
    }

    /**
     * @param  array<string, mixed>  $sourcePayload
     */
    protected function extractScore(array $sourcePayload): float
    {
        $score = $sourcePayload['score'] ?? 0;

        return is_numeric($score) ? (float) $score : 0.0;
    }

    protected function timestampValue(mixed $value): int
    {
        if ($value instanceof CarbonInterface) {
            return $value->getTimestamp();
        }

        if (is_string($value) && $value !== '') {
            return CarbonImmutable::parse($value)->getTimestamp();
        }

        return 0;
    }

    protected function resolveStateFromCount(int $count): string
    {
        return $count > 0 ? 'ok' : 'empty';
    }

    /**
     * @return Collection<int, array<string, mixed>>
     */
    protected function getSystemDistributionSummaryCandidates(?int $minimumDistinctUserCount = null): Collection
    {
        $minimumDistinctUserCount = max(
            $minimumDistinctUserCount ?? (int) config('personalized_project_recommendations.distribution_summary.minimum_distinct_user_count', 2),
            1,
        );
        $eligibleSourceTypes = collect((array) config('personalized_project_recommendations.distribution_summary.eligible_source_types', []))
            ->map(fn (mixed $value): string => trim((string) $value))
            ->filter()
            ->unique()
            ->values()
            ->all();

        if ($eligibleSourceTypes === []) {
            return collect();
        }

        $rows = SystemRecommendationAssignment::query()
            ->selectRaw('source_type, source_reference, subject_id, sub_subject_id, COUNT(DISTINCT user_id) as distinct_user_count, MAX(last_distributed_at) as latest_distribution_at')
            ->whereIn('source_type', $eligibleSourceTypes)
            ->whereNotNull('source_reference')
            ->whereNotNull('sub_subject_id')
            ->groupBy('source_type', 'source_reference', 'subject_id', 'sub_subject_id')
            ->havingRaw('COUNT(DISTINCT user_id) >= ?', [$minimumDistinctUserCount])
            ->get();

        if ($rows->isEmpty()) {
            return collect();
        }

        $subSubjectLookup = SubSubject::query()
            ->with('subject')
            ->whereIn(
                'id',
                $rows
                    ->pluck('sub_subject_id')
                    ->filter()
                    ->map(fn (mixed $id): int => (int) $id)
                    ->unique()
                    ->values()
                    ->all(),
            )
            ->get()
            ->keyBy('id');
        $sourceMetadataLookup = $this->buildSystemDistributionSourceMetadataLookup($rows);

        return $rows
            ->map(function (SystemRecommendationAssignment $row) use ($subSubjectLookup, $sourceMetadataLookup): array {
                $subSubject = $subSubjectLookup->get((int) $row->sub_subject_id);
                $subject = $subSubject?->subject;
                $sourceType = (string) $row->source_type;
                $sourceReference = (string) $row->source_reference;
                $sourceMetadata = $this->resolveSystemDistributionSourceMetadata(
                    $sourceType,
                    $sourceReference,
                    $sourceMetadataLookup,
                );

                return [
                    'recommendation_key' => $this->makeSourceKey($sourceType, $sourceReference),
                    'recommendation_item_id' => $sourceMetadata['recommendation_item_id'],
                    'title' => $sourceMetadata['title'] ?? ($sourceType . ':' . $sourceReference),
                    'source_type' => $sourceType,
                    'source_reference' => $sourceReference,
                    'subject_id' => $subject?->id ?? (is_numeric($row->subject_id) ? (int) $row->subject_id : null),
                    'sub_subject_id' => (int) $row->sub_subject_id,
                    'subject' => $subject ? [
                        'id' => $subject->id,
                        'name' => $subject->name,
                        'slug' => $subject->slug,
                    ] : null,
                    'sub_subject' => $subSubject ? [
                        'id' => $subSubject->id,
                        'subject_id' => $subSubject->subject_id,
                        'name' => $subSubject->name,
                        'slug' => $subSubject->slug,
                    ] : null,
                    'distinct_user_count' => (int) $row->distinct_user_count,
                    'latest_distribution_at' => $this->normalizeTimestampValue($row->latest_distribution_at),
                    'source_created_at' => $sourceMetadata['source_created_at'],
                ];
            })
            ->filter(fn (array $row) => $row['sub_subject'] !== null)
            ->values();
    }

    /**
     * @param  Collection<int, SystemRecommendationAssignment>  $rows
     * @return array<string, array<string, mixed>>
     */
    protected function buildSystemDistributionSourceMetadataLookup(Collection $rows): array
    {
        $aiGeneratedReferences = $rows
            ->where('source_type', RecommendedProject::SOURCE_AI_GENERATED)
            ->pluck('source_reference')
            ->filter()
            ->map(fn (mixed $value): string => (string) $value)
            ->unique()
            ->values()
            ->all();
        $systemTopicReferences = $rows
            ->where('source_type', RecommendedProject::SOURCE_SYSTEM_TOPIC)
            ->pluck('source_reference')
            ->filter()
            ->map(fn (mixed $value): string => (string) $value)
            ->unique()
            ->values()
            ->all();

        return [
            RecommendedProject::SOURCE_AI_GENERATED => RecommendedProject::query()
                ->where('source_type', RecommendedProject::SOURCE_AI_GENERATED)
                ->whereIn('id', $aiGeneratedReferences)
                ->get()
                ->keyBy(fn (RecommendedProject $project): string => (string) $project->id)
                ->all(),
            'system_topic_overrides' => RecommendedProject::query()
                ->where('source_type', RecommendedProject::SOURCE_SYSTEM_TOPIC)
                ->whereIn('source_reference', $systemTopicReferences)
                ->orderByDesc('created_at')
                ->orderByDesc('id')
                ->get()
                ->groupBy(fn (RecommendedProject $project): string => (string) $project->source_reference)
                ->map(fn (Collection $items): ?RecommendedProject => $items->first())
                ->all(),
            RecommendedProject::SOURCE_SYSTEM_TOPIC => Topic::query()
                ->select(['id', 'title', 'created_at'])
                ->whereIn('id', $systemTopicReferences)
                ->get()
                ->keyBy(fn (Topic $topic): string => (string) $topic->id)
                ->all(),
        ];
    }

    /**
     * @param  array<string, array<string, mixed>>  $sourceMetadataLookup
     * @return array{recommendation_item_id: string, title: string|null, source_created_at: CarbonInterface|string|null}
     */
    protected function resolveSystemDistributionSourceMetadata(
        string $sourceType,
        string $sourceReference,
        array $sourceMetadataLookup,
    ): array {
        if ($sourceType === RecommendedProject::SOURCE_AI_GENERATED) {
            /** @var RecommendedProject|null $project */
            $project = data_get($sourceMetadataLookup, RecommendedProject::SOURCE_AI_GENERATED . '.' . $sourceReference);

            return [
                'recommendation_item_id' => $project ? (string) $project->id : $sourceReference,
                'title' => $project?->title,
                'source_created_at' => $project?->created_at,
            ];
        }

        if ($sourceType === RecommendedProject::SOURCE_SYSTEM_TOPIC) {
            /** @var RecommendedProject|null $override */
            $override = data_get($sourceMetadataLookup, 'system_topic_overrides.' . $sourceReference);

            if ($override) {
                return [
                    'recommendation_item_id' => (string) $override->id,
                    'title' => $override->title,
                    'source_created_at' => $override->created_at,
                ];
            }

            /** @var Topic|null $topic */
            $topic = data_get($sourceMetadataLookup, RecommendedProject::SOURCE_SYSTEM_TOPIC . '.' . $sourceReference);

            return [
                'recommendation_item_id' => $topic ? 'system_topic_' . $topic->id : 'system_topic_' . $sourceReference,
                'title' => $topic?->title,
                'source_created_at' => $topic?->created_at,
            ];
        }

        return [
            'recommendation_item_id' => $sourceReference,
            'title' => null,
            'source_created_at' => null,
        ];
    }

    protected function compareSystemDistributionCandidates(array $left, array $right): int
    {
        $tieBreakers = (array) config('personalized_project_recommendations.distribution_summary.tie_breakers', [
            'distinct_user_count' => 'desc',
            'latest_distribution_at' => 'desc',
            'source_created_at' => 'desc',
            'source_reference' => 'asc',
        ]);

        foreach ($tieBreakers as $field => $direction) {
            $comparison = $this->compareSystemDistributionField($left, $right, (string) $field, (string) $direction);

            if ($comparison !== 0) {
                return $comparison;
            }
        }

        return strcmp((string) data_get($left, 'source_type'), (string) data_get($right, 'source_type'))
            ?: strcmp((string) data_get($left, 'source_reference'), (string) data_get($right, 'source_reference'))
            ?: strcmp((string) data_get($left, 'title'), (string) data_get($right, 'title'));
    }

    protected function compareSystemDistributionField(array $left, array $right, string $field, string $direction): int
    {
        $direction = strtolower($direction) === 'asc' ? 'asc' : 'desc';

        $comparison = match ($field) {
            'distinct_user_count' => ((int) data_get($left, $field, 0)) <=> ((int) data_get($right, $field, 0)),
            'latest_distribution_at', 'source_created_at' => $this->timestampValue(data_get($left, $field))
                <=> $this->timestampValue(data_get($right, $field)),
            default => strcmp((string) data_get($left, $field, ''), (string) data_get($right, $field, '')),
        };

        return $direction === 'asc' ? $comparison : ($comparison * -1);
    }

    protected function normalizeTimestampValue(mixed $value): CarbonInterface|string|null
    {
        if ($value instanceof CarbonInterface) {
            return $value;
        }

        return is_string($value) && $value !== ''
            ? CarbonImmutable::parse($value)
            : null;
    }

    /**
     * @param  array<string, mixed>  $item
     * @return array<string, mixed>
     */
    protected function normalizeAdminSystemDistributionSummaryItem(array $item): array
    {
        return [
            'title' => (string) data_get($item, 'title', ''),
            'subject' => [
                'id' => data_get($item, 'subject.id'),
                'name' => data_get($item, 'subject.name'),
                'slug' => data_get($item, 'subject.slug'),
            ],
            'sub_subject' => [
                'id' => data_get($item, 'sub_subject.id'),
                'subject_id' => data_get($item, 'sub_subject.subject_id'),
                'name' => data_get($item, 'sub_subject.name'),
                'slug' => data_get($item, 'sub_subject.slug'),
            ],
            'source_type' => (string) data_get($item, 'source_type', ''),
            'source_reference' => data_get($item, 'source_reference'),
            'distinct_user_count' => (int) data_get($item, 'distinct_user_count', 0),
            'latest_distribution_at' => $this->serializeTimestampForAdminUi(data_get($item, 'latest_distribution_at')),
        ];
    }

    protected function serializeTimestampForAdminUi(mixed $value): ?string
    {
        if ($value instanceof CarbonInterface) {
            return $value->toISOString();
        }

        if (is_string($value) && $value !== '') {
            return CarbonImmutable::parse($value)->toISOString();
        }

        return null;
    }
}