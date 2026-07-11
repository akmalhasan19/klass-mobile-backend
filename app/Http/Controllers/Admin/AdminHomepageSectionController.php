<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use App\Models\HomepageSection;
use App\Models\RecommendedProject;
use App\Services\RecommendationAggregationService;
use Carbon\CarbonImmutable;
use Illuminate\Http\Request;
use Illuminate\Http\RedirectResponse;
use Illuminate\Support\Facades\DB;
use Illuminate\View\View;
use Throwable;

class AdminHomepageSectionController extends Controller
{
    public function __construct(
        protected RecommendationAggregationService $recommendationAggregationService,
    ) {
    }

    /**
     * Tampilkan konfigurasi homepage sections.
     */
    public function index(Request $request): View
    {
        $homepageSections = HomepageSection::query()
            ->orderBy('position')
            ->orderBy('label')
            ->get();
        $recommendedProjects = $this->loadRecommendedProjects($request);
        $discoveryLock = $this->buildDiscoveryLock();
        $systemDistributionSummary = $this->loadSystemDistributionSummaryViewModel();

        return view('admin.homepage-sections.index', compact('homepageSections', 'recommendedProjects', 'discoveryLock', 'systemDistributionSummary'));
    }

    protected function loadRecommendedProjects(Request $request)
    {
        $query = RecommendedProject::query();

        if ($request->filled('source_type')) {
            $query->where('source_type', $request->source_type);
        }

        if ($request->filled('status')) {
            $now = now();

            switch ($request->status) {
                case 'active':
                    $query->where('is_active', true)
                        ->where(function ($builder) use ($now) {
                            $builder->whereNull('starts_at')->orWhere('starts_at', '<=', $now);
                        })
                        ->where(function ($builder) use ($now) {
                            $builder->whereNull('ends_at')->orWhere('ends_at', '>=', $now);
                        });
                    break;
                case 'inactive':
                    $query->where('is_active', false);
                    break;
                case 'scheduled':
                    $query->where('is_active', true)->where('starts_at', '>', $now);
                    break;
                case 'expired':
                    $query->where('is_active', true)->where('ends_at', '<', $now);
                    break;
            }
        }

        return $query
            ->orderBy('display_priority', 'desc')
            ->orderByDesc('id')
            ->get();
    }

    /**
     * @return array{items: array<int, array<string, mixed>>, empty_state: array<string, mixed>, items_count: int}
     */
    protected function loadSystemDistributionSummaryViewModel(): array
    {
        try {
            $payload = $this->recommendationAggregationService->buildAdminSystemDistributionSummaryPayload();
        } catch (Throwable $throwable) {
            report($throwable);

            $payload = [
                'items' => [],
                'empty_state' => [
                    'is_empty' => true,
                    'message' => (string) config(
                        'personalized_project_recommendations.homepage.admin_sections.system_distribution_empty_state',
                        'No system recommendation has been distributed to more than one user yet.',
                    ),
                ],
            ];
        }

        $items = collect($payload['items'] ?? [])
            ->map(function (array $item): array {
                $latestDistributionAt = $this->formatAdminSummaryTimestamp(data_get($item, 'latest_distribution_at'));

                return array_merge($item, [
                    'subject_label' => (string) (data_get($item, 'subject.name') ?: 'Unassigned subject'),
                    'sub_subject_label' => (string) (data_get($item, 'sub_subject.name') ?: 'Unassigned sub-subject'),
                    'latest_distribution_at_label' => $latestDistributionAt,
                    'source_label' => strtoupper(str_replace('_', ' ', (string) data_get($item, 'source_type', 'unknown'))),
                ]);
            })
            ->values()
            ->all();

        return [
            'items' => $items,
            'empty_state' => [
                'is_empty' => $items === [],
                'message' => (string) data_get(
                    $payload,
                    'empty_state.message',
                    config(
                        'personalized_project_recommendations.homepage.admin_sections.system_distribution_empty_state',
                        'No system recommendation has been distributed to more than one user yet.',
                    ),
                ),
            ],
            'items_count' => count($items),
        ];
    }

    public function update(Request $request): RedirectResponse
    {
        $validated = $request->validate([
            'sections' => 'required|array|min:1',
            'sections.*.id' => 'required|string|exists:homepage_sections,id',
            'sections.*.label' => 'required|string|max:255',
            'sections.*.position' => 'required|integer|min:1',
            'sections.*.is_enabled' => 'sometimes|boolean',
        ]);

        $sections = collect($validated['sections']);
        $sectionIds = $sections->pluck('id')->values()->all();

        DB::transaction(function () use ($sections, $sectionIds): void {
            $storedSections = HomepageSection::query()
                ->whereIn('id', $sectionIds)
                ->get()
                ->keyBy('id');

            foreach ($sections as $sectionPayload) {
                $section = $storedSections->get($sectionPayload['id']);

                if (! $section) {
                    continue;
                }

                $section->update([
                    'label' => $sectionPayload['label'],
                    'position' => (int) $sectionPayload['position'],
                    'is_enabled' => array_key_exists('is_enabled', $sectionPayload)
                        ? (bool) $sectionPayload['is_enabled']
                        : false,
                ]);
            }
        });

        ActivityLog::create([
            'actor_id' => $request->user()?->id,
            'action' => 'update_homepage_sections',
            'subject_type' => HomepageSection::class,
            'subject_id' => 'bulk',
            'metadata' => [
                'section_ids' => $sectionIds,
            ],
        ]);

        return redirect()
            ->route('admin.homepage-sections.index')
            ->with('success', 'Homepage sections updated successfully.');
    }

    /**
     * @return array<string, mixed>
     */
    protected function buildDiscoveryLock(): array
    {
        $tieBreakers = [];

        foreach ((array) config('personalized_project_recommendations.distribution_summary.tie_breakers', []) as $field => $direction) {
            $tieBreakers[] = sprintf('%s %s', $field, strtoupper((string) $direction));
        }

        return [
            'curated_title' => (string) config('personalized_project_recommendations.homepage.admin_sections.curated_title', 'Recommended Projects (Admin Curated)'),
            'system_section_title' => (string) config('personalized_project_recommendations.homepage.admin_sections.system_distribution_title', 'Top Distributed System Recommendations by Sub-Subject'),
            'system_section_description' => (string) config('personalized_project_recommendations.homepage.admin_sections.system_distribution_description', ''),
            'system_section_empty_state' => (string) config('personalized_project_recommendations.homepage.admin_sections.system_distribution_empty_state', ''),
            'feed_endpoint' => (string) config('personalized_project_recommendations.homepage.feed_endpoint', '/api/v1/homepage-recommendations'),
            'admin_configurator_path' => (string) config('personalized_project_recommendations.homepage.admin_configurator_path', '/admin/homepage-sections'),
            'eligible_source_types' => array_values(array_filter((array) config('personalized_project_recommendations.distribution_summary.eligible_source_types', []))),
            'minimum_distinct_user_count' => (int) config('personalized_project_recommendations.distribution_summary.minimum_distinct_user_count', 2),
            'maximum_items_per_sub_subject' => (int) config('personalized_project_recommendations.distribution_summary.maximum_items_per_sub_subject', 1),
            'tie_breakers' => $tieBreakers,
            'authenticated_fallback' => (string) config('personalized_project_recommendations.fallbacks.authenticated_without_personalization.description', ''),
            'guest_fallback' => (string) config('personalized_project_recommendations.fallbacks.guest.description', ''),
        ];
    }

    protected function formatAdminSummaryTimestamp(mixed $value): string
    {
        if (! is_string($value) || $value === '') {
            return 'Not distributed yet';
        }

        return CarbonImmutable::parse($value)
            ->setTimezone(config('app.timezone'))
            ->format('d M Y, H:i');
    }
}
