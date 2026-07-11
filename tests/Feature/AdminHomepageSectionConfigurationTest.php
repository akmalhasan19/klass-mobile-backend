<?php

namespace Tests\Feature;

use App\Models\HomepageSection;
use App\Models\RecommendedProject;
use App\Models\SubSubject;
use App\Models\SystemRecommendationAssignment;
use App\Models\User;
use Carbon\CarbonImmutable;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminHomepageSectionConfigurationTest extends TestCase
{
    use RefreshDatabase;

    public function test_admin_can_update_homepage_section_configuration_and_mobile_api_uses_it(): void
    {
        $admin = User::factory()->admin()->create();

        $projects = HomepageSection::create([
            'key' => 'project_recommendations',
            'label' => 'Project Recommendations',
            'position' => 1,
            'is_enabled' => true,
            'data_source' => 'topics',
        ]);

        $freelancers = HomepageSection::create([
            'key' => 'top_freelancers',
            'label' => 'Top Freelancers',
            'position' => 2,
            'is_enabled' => true,
            'data_source' => 'marketplace_tasks',
        ]);

        $archived = HomepageSection::create([
            'key' => 'archived_section',
            'label' => 'Archived Section',
            'position' => 3,
            'is_enabled' => true,
            'data_source' => 'legacy',
        ]);

        $this->actingAs($admin)
            ->get(route('admin.homepage-sections.index'))
            ->assertOk()
            ->assertViewHas('systemDistributionSummary', function (array $summary): bool {
                return ($summary['empty_state']['is_empty'] ?? null) === true
                    && ($summary['items_count'] ?? null) === 0
                    && ($summary['empty_state']['message'] ?? null) === 'No system recommendation has been distributed to more than one user yet.'
                    && ($summary['items'] ?? []) === [];
            })
            ->assertSeeText('Homepage Configurator')
            ->assertSeeText('Curate the mobile experience: manage sections and recommended projects.')
            ->assertSeeText('Recommended Projects (Admin Curated)')
            ->assertSeeText('Top Distributed System Recommendations by Sub-Subject')
            ->assertSeeText('GET /api/v1/homepage-recommendations')
            ->assertSeeText('No system recommendation has been distributed to more than one user yet.')
            ->assertSeeText('The summary will appear here after a system recommendation has been distributed to more than one distinct user.')
            ->assertSeeInOrder([
                'Recommended Projects (Admin Curated)',
                'Top Distributed System Recommendations by Sub-Subject',
            ])
            ->assertSeeText('Section Ordering');

        $this->actingAs($admin)
            ->patch(route('admin.homepage-sections.update'), [
                'sections' => [
                    [
                        'id' => $freelancers->id,
                        'label' => 'Freelancer Pilihan',
                        'position' => 1,
                        'is_enabled' => true,
                    ],
                    [
                        'id' => $projects->id,
                        'label' => 'Belajar Minggu Ini',
                        'position' => 2,
                        'is_enabled' => true,
                    ],
                    [
                        'id' => $archived->id,
                        'label' => 'Archived Section',
                        'position' => 3,
                    ],
                ],
            ])
            ->assertRedirect();

        $this->assertDatabaseHas('homepage_sections', [
            'id' => $freelancers->id,
            'label' => 'Freelancer Pilihan',
            'position' => 1,
            'is_enabled' => true,
        ]);

        $this->assertDatabaseHas('homepage_sections', [
            'id' => $projects->id,
            'label' => 'Belajar Minggu Ini',
            'position' => 2,
            'is_enabled' => true,
        ]);

        $this->assertDatabaseHas('homepage_sections', [
            'id' => $archived->id,
            'is_enabled' => false,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'update_homepage_sections',
            'subject_type' => HomepageSection::class,
            'subject_id' => 'bulk',
        ]);

        $this->getJson('/api/v1/homepage-sections')
            ->assertOk()
            ->assertJsonCount(2, 'data')
            ->assertJsonPath('data.0.key', 'top_freelancers')
            ->assertJsonPath('data.0.label', 'Freelancer Pilihan')
            ->assertJsonPath('data.1.key', 'project_recommendations')
            ->assertJsonPath('data.1.label', 'Belajar Minggu Ini');
    }

    public function test_admin_homepage_configurator_receives_system_distribution_summary_output_contract(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $admin = User::factory()->admin()->create();

        HomepageSection::create([
            'key' => 'project_recommendations',
            'label' => 'Project Recommendations',
            'position' => 1,
            'is_enabled' => true,
            'data_source' => 'topics',
        ]);

        $science = \App\Models\Subject::query()->where('slug', 'science')->firstOrFail();
        $thermodynamics = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();
        $project = RecommendedProject::factory()->create([
            'title' => 'Thermodynamics Distribution Winner',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $science->id,
                'sub_subject_id' => $thermodynamics->id,
            ],
        ]);
        $latestDistributionAt = CarbonImmutable::parse('2026-04-07 13:30:00');

        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $project->id,
            $science->id,
            $thermodynamics->id,
            $latestDistributionAt->subMinutes(10),
        );
        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $project->id,
            $science->id,
            $thermodynamics->id,
            $latestDistributionAt,
        );

        $this->actingAs($admin)
            ->get(route('admin.homepage-sections.index'))
            ->assertOk()
            ->assertViewHas('systemDistributionSummary', function (array $summary) use ($project, $latestDistributionAt): bool {
                if (($summary['empty_state']['is_empty'] ?? true) !== false) {
                    return false;
                }

                if (($summary['empty_state']['message'] ?? null) !== 'No system recommendation has been distributed to more than one user yet.') {
                    return false;
                }

                if (count($summary['items'] ?? []) !== 1) {
                    return false;
                }

                if (($summary['items_count'] ?? null) !== 1) {
                    return false;
                }

                $item = $summary['items'][0];

                return ($item['title'] ?? null) === 'Thermodynamics Distribution Winner'
                    && ($item['subject']['slug'] ?? null) === 'science'
                    && ($item['sub_subject']['slug'] ?? null) === 'thermodynamics'
                    && ($item['source_type'] ?? null) === RecommendedProject::SOURCE_AI_GENERATED
                    && ($item['source_reference'] ?? null) === (string) $project->id
                    && ($item['distinct_user_count'] ?? null) === 2
                    && ($item['latest_distribution_at'] ?? null) === $latestDistributionAt->toISOString()
                    && ($item['subject_label'] ?? null) === 'Science'
                    && ($item['sub_subject_label'] ?? null) === 'Thermodynamics';
            })
            ->assertSeeText('Thermodynamics Distribution Winner')
            ->assertSeeText('Thermodynamics')
            ->assertSeeText('Science')
            ->assertSeeText('2 users')
            ->assertSeeText('AI GENERATED');
    }

    public function test_admin_homepage_configurator_keeps_curated_actions_visible_when_system_summary_exists(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $admin = User::factory()->admin()->create();

        HomepageSection::create([
            'key' => 'project_recommendations',
            'label' => 'Project Recommendations',
            'position' => 1,
            'is_enabled' => true,
            'data_source' => 'topics',
        ]);

        $curatedProject = RecommendedProject::factory()->create([
            'title' => 'Admin Curated Project',
            'source_type' => RecommendedProject::SOURCE_ADMIN_UPLOAD,
            'is_active' => true,
        ]);

        $science = \App\Models\Subject::query()->where('slug', 'science')->firstOrFail();
        $thermodynamics = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();
        $systemProject = RecommendedProject::factory()->create([
            'title' => 'System Summary Winner',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $science->id,
                'sub_subject_id' => $thermodynamics->id,
            ],
        ]);

        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $systemProject->id,
            $science->id,
            $thermodynamics->id,
            CarbonImmutable::parse('2026-04-07 10:00:00'),
        );
        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $systemProject->id,
            $science->id,
            $thermodynamics->id,
            CarbonImmutable::parse('2026-04-07 11:00:00'),
        );

        $this->actingAs($admin)
            ->get(route('admin.homepage-sections.index'))
            ->assertOk()
            ->assertSeeText('+ Add Project')
            ->assertSeeText('Admin Curated Project')
            ->assertSeeText('Edit')
            ->assertSeeText('Deactivate')
            ->assertSeeText('Tampilkan Sekarang')
            ->assertSeeText('Delete')
            ->assertSeeText('System Summary Winner');
    }

    protected function createSystemRecommendationAssignment(
        User $user,
        string $sourceType,
        string $sourceReference,
        int $subjectId,
        int $subSubjectId,
        CarbonImmutable $distributedAt,
    ): void {
        SystemRecommendationAssignment::create([
            'user_id' => $user->id,
            'recommendation_key' => $sourceType . ':' . $sourceReference,
            'recommendation_item_id' => $sourceReference,
            'source_type' => $sourceType,
            'source_reference' => $sourceReference,
            'subject_id' => $subjectId,
            'sub_subject_id' => $subSubjectId,
            'first_distributed_at' => $distributedAt,
            'last_distributed_at' => $distributedAt,
        ]);
    }
}