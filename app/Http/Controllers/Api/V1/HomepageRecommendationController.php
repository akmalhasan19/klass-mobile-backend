<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\HomepageRecommendationRequest;
use App\Http\Resources\RecommendedProjectRecommendationCollection;
use App\Models\HomepageSection;
use App\Models\RecommendedProject;
use App\Models\User;
use App\Services\RecommendationAggregationService;
use App\Services\RecommendationPersonalizationService;
use App\Services\SystemRecommendationAssignmentService;
use Illuminate\Http\Request;
use Illuminate\Support\Collection;
use Throwable;

class HomepageRecommendationController extends Controller
{
    public function __construct(
        protected RecommendationAggregationService $recommendationAggregationService,
        protected RecommendationPersonalizationService $recommendationPersonalizationService,
        protected SystemRecommendationAssignmentService $systemRecommendationAssignmentService,
    ) {
    }

    public function index(HomepageRecommendationRequest $request): RecommendedProjectRecommendationCollection
    {
        $validated = $request->validated();

        $section = HomepageSection::query()
            ->where('key', $this->sectionKey())
            ->first();

        $requestedLimit = isset($validated['limit']) ? (int) $validated['limit'] : null;
        $user = auth('sanctum')->user() ?? $request->user();
        $personalizationContext = $this->recommendationPersonalizationService->resolve($user);

        if ($section !== null && ! $section->is_enabled) {
            return (new RecommendedProjectRecommendationCollection(collect()))
                ->withContextMeta([
                    'section' => $this->buildSectionMeta($section),
                    'limit' => [
                        'requested' => $requestedLimit,
                        'applied' => 0,
                    ],
                    'personalization' => $this->buildPersonalizationMeta($request, $personalizationContext),
                    'source_status' => $this->notEvaluatedSourceStatus(),
                ]);
        }

        $snapshot = $this->recommendationAggregationService->buildFeedSnapshot(
            personalizationContext: $personalizationContext,
        );
        $items = $snapshot['items'];

        if ($requestedLimit !== null) {
            $items = $items->take($requestedLimit)->values();
        }

        if ($user instanceof User) {
            $this->trackSystemRecommendationAssignments($user, $items);
        }

        return (new RecommendedProjectRecommendationCollection($items))
            ->withContextMeta([
                'section' => $this->buildSectionMeta($section),
                'limit' => [
                    'requested' => $requestedLimit,
                    'applied' => $items->count(),
                ],
                'personalization' => $this->buildPersonalizationMeta(
                    $request,
                    $personalizationContext,
                    (array) ($snapshot['personalization'] ?? []),
                ),
                'source_status' => $snapshot['source_status'],
            ]);
    }

    protected function buildSectionMeta(?HomepageSection $section): array
    {
        return [
            'key' => $this->sectionKey(),
            'label' => $section?->label,
            'enabled' => (bool) $section?->is_enabled,
            'position' => $section?->position,
            'endpoint' => $this->feedEndpoint(),
            'admin_configurator_path' => $this->adminConfiguratorPath(),
        ];
    }

    protected function buildPersonalizationMeta(
        Request $request,
        ?array $personalizationContext = null,
        array $appliedPersonalizationMeta = [],
    ): array
    {
        $user = auth('sanctum')->user() ?? $request->user();
        $policyKey = $user ? 'authenticated_without_personalization' : 'guest';
        $policy = (array) config("personalized_project_recommendations.fallbacks.{$policyKey}", []);

        return array_replace_recursive([
            'policy_version' => (string) config('personalized_project_recommendations.lock_version', 'phase_0_discovery_lock'),
            'audience' => $user ? 'authenticated' : 'guest',
            'mode' => (string) ($policy['mode'] ?? 'default_global_feed'),
            'tracks_assignments' => (bool) ($policy['tracks_assignments'] ?? false),
            'description' => (string) ($policy['description'] ?? ''),
            'topic_guardrails' => [
                'taxonomy_required_for_personalization' => (bool) config('personalized_project_recommendations.topic_guardrails.taxonomy_required_for_personalization', true),
                'missing_sub_subject_fallback' => (string) config('personalized_project_recommendations.topic_guardrails.missing_sub_subject_fallback', 'general_feed_only'),
                'allow_unresolved_ownership_in_general_feed' => (bool) config('personalized_project_recommendations.topic_guardrails.allow_unresolved_ownership_in_general_feed', true),
                'unresolved_ownership_fallback' => (string) config('personalized_project_recommendations.topic_guardrails.unresolved_ownership_fallback', 'general_feed_only'),
            ],
        ], (array) data_get($personalizationContext, 'public', []), $appliedPersonalizationMeta);
    }

    protected function sectionKey(): string
    {
        return (string) config('personalized_project_recommendations.homepage.section_key', 'project_recommendations');
    }

    protected function feedEndpoint(): string
    {
        return (string) config('personalized_project_recommendations.homepage.feed_endpoint', '/api/v1/homepage-recommendations');
    }

    protected function adminConfiguratorPath(): string
    {
        return (string) config('personalized_project_recommendations.homepage.admin_configurator_path', '/admin/homepage-sections');
    }

    /**
     * @param  Collection<int, array<string, mixed>>  $items
     */
    protected function trackSystemRecommendationAssignments(User $user, Collection $items): void
    {
        try {
            $this->systemRecommendationAssignmentService->trackServedRecommendations($user, $items);
        } catch (Throwable $throwable) {
            report($throwable);
        }
    }

    /**
     * @return array<string, array<string, string>>
     */
    protected function notEvaluatedSourceStatus(): array
    {
        return [
            RecommendedProject::SOURCE_ADMIN_UPLOAD => ['state' => 'not_evaluated'],
            RecommendedProject::SOURCE_SYSTEM_TOPIC => ['state' => 'not_evaluated'],
            RecommendedProject::SOURCE_AI_GENERATED => ['state' => 'not_evaluated'],
        ];
    }
}