<?php

namespace Tests\Feature;

use App\Models\HomepageSection;
use App\Models\RecommendedProject;
use App\Models\SubSubject;
use App\Models\SystemRecommendationAssignment;
use App\Models\Topic;
use App\Models\User;
use Carbon\CarbonImmutable;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class Phase7EndToEndVerificationTest extends TestCase
{
    use RefreshDatabase;

    public function test_phase_seven_manual_verification_flow_covers_guest_authenticated_refresh_and_admin_summary_selection(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        HomepageSection::create([
            'key' => 'project_recommendations',
            'label' => 'Project Recommendations',
            'position' => 1,
            'is_enabled' => true,
            'data_source' => 'recommended_projects',
        ]);

        $science = \App\Models\Subject::query()->where('slug', 'science')->firstOrFail();
        $quantumPhysics = SubSubject::query()->where('slug', 'quantum-physics')->firstOrFail();
        $thermodynamics = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();
        $algebra = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $history = SubSubject::query()->where('slug', 'indonesian-history')->firstOrFail();

        $viewer = User::factory()->teacher()->create([
            'primary_subject_id' => $science->id,
        ]);
        $historyTeacher = User::factory()->teacher()->create();

        RecommendedProject::factory()->create([
            'title' => 'Admin Curated Anchor',
            'display_priority' => 100,
            'source_type' => RecommendedProject::SOURCE_ADMIN_UPLOAD,
        ]);

        $quantumActivityTopic = Topic::create([
            'title' => 'Quantum Activity Topic',
            'teacher_id' => (string) $viewer->id,
            'sub_subject_id' => $quantumPhysics->id,
            'is_published' => true,
            'order' => 1,
        ]);

        Topic::create([
            'title' => 'Algebra Activity Topic',
            'teacher_id' => (string) $viewer->id,
            'sub_subject_id' => $algebra->id,
            'is_published' => true,
            'order' => 2,
        ]);

        $thermodynamicsWinner = RecommendedProject::factory()->create([
            'title' => 'Thermodynamics Winner Candidate',
            'display_priority' => 30,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'score' => 7.5,
                'subject_id' => $science->id,
                'sub_subject_id' => $thermodynamics->id,
            ],
        ]);

        $thermodynamicsLoser = RecommendedProject::factory()->create([
            'title' => 'Thermodynamics Lower Distribution Candidate',
            'display_priority' => 20,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'is_active' => false,
            'source_payload' => [
                'score' => 6.9,
                'subject_id' => $science->id,
                'sub_subject_id' => $thermodynamics->id,
            ],
        ]);

        Topic::create([
            'title' => 'History General Topic',
            'teacher_id' => (string) $historyTeacher->id,
            'sub_subject_id' => $history->id,
            'is_published' => true,
            'order' => 3,
        ]);

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.personalization.audience', 'guest')
            ->assertJsonPath('meta.personalization.mode', 'default_global_feed')
            ->assertJsonPath('meta.personalization.tracks_assignments', false)
            ->assertJsonPath('data.0.title', 'Admin Curated Anchor');

        Sanctum::actingAs($viewer);

        $firstMoment = CarbonImmutable::parse('2026-04-07 14:00:00');
        $secondMoment = CarbonImmutable::parse('2026-04-07 14:30:00');

        $this->travelTo($firstMoment);

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.personalization.audience', 'authenticated')
            ->assertJsonPath('meta.personalization.applied', true)
            ->assertJsonPath('meta.personalization.subject_anchor.slug', 'science')
            ->assertJsonPath('data.0.title', 'Admin Curated Anchor')
            ->assertJsonPath('data.1.title', 'Quantum Activity Topic')
            ->assertJsonPath('data.2.title', 'Thermodynamics Winner Candidate')
            ->assertJsonPath('data.3.title', 'Algebra Activity Topic')
            ->assertJsonMissing(['title' => 'History General Topic']);

        $this->assertDatabaseCount('system_recommendation_assignments', 3);

        $winnerAssignment = SystemRecommendationAssignment::query()
            ->where('user_id', $viewer->id)
            ->where('recommendation_key', RecommendedProject::SOURCE_AI_GENERATED . ':' . $thermodynamicsWinner->id)
            ->firstOrFail();

        $this->assertTrue($winnerAssignment->first_distributed_at->equalTo($firstMoment));
        $this->assertTrue($winnerAssignment->last_distributed_at->equalTo($firstMoment));

        $this->travelTo($secondMoment);

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.personalization.applied', true)
            ->assertJsonPath('meta.personalization.tracks_assignments', true);

        $this->assertDatabaseCount('system_recommendation_assignments', 3);

        $winnerAssignment->refresh();
        $this->assertTrue($winnerAssignment->first_distributed_at->equalTo($firstMoment));
        $this->assertTrue($winnerAssignment->last_distributed_at->equalTo($secondMoment));

        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $thermodynamicsWinner->id,
            $science->id,
            $thermodynamics->id,
            CarbonImmutable::parse('2026-04-07 15:00:00'),
        );
        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $thermodynamicsWinner->id,
            $science->id,
            $thermodynamics->id,
            CarbonImmutable::parse('2026-04-07 15:15:00'),
        );
        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $thermodynamicsLoser->id,
            $science->id,
            $thermodynamics->id,
            CarbonImmutable::parse('2026-04-07 15:20:00'),
        );
        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $thermodynamicsLoser->id,
            $science->id,
            $thermodynamics->id,
            CarbonImmutable::parse('2026-04-07 15:25:00'),
        );
        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_SYSTEM_TOPIC,
            $quantumActivityTopic->id,
            $science->id,
            $quantumPhysics->id,
            CarbonImmutable::parse('2026-04-07 15:30:00'),
        );

        $admin = User::factory()->admin()->create();

        $this->actingAs($admin)
            ->get(route('admin.homepage-sections.index'))
            ->assertOk()
            ->assertViewHas('systemDistributionSummary', function (array $summary) use ($quantumActivityTopic, $quantumPhysics, $thermodynamics, $thermodynamicsWinner): bool {
                $items = collect($summary['items'] ?? []);

                if (($summary['items_count'] ?? null) !== 2) {
                    return false;
                }

                if ($items->count() !== $items->pluck('sub_subject.id')->unique()->count()) {
                    return false;
                }

                $thermodynamicsItem = $items->firstWhere('sub_subject.id', $thermodynamics->id);
                $quantumItem = $items->firstWhere('sub_subject.id', $quantumPhysics->id);

                return ($thermodynamicsItem['title'] ?? null) === 'Thermodynamics Winner Candidate'
                    && ($thermodynamicsItem['source_reference'] ?? null) === (string) $thermodynamicsWinner->id
                    && ($thermodynamicsItem['distinct_user_count'] ?? null) === 3
                    && ($quantumItem['title'] ?? null) === $quantumActivityTopic->title
                    && ($quantumItem['source_reference'] ?? null) === $quantumActivityTopic->id
                    && ($quantumItem['distinct_user_count'] ?? null) === 2;
            })
            ->assertSeeText('Thermodynamics Winner Candidate')
            ->assertSeeText('Quantum Activity Topic')
            ->assertSeeText('3 users')
            ->assertSeeText('2 users');

        $this->travelBack();
    }

    public function test_phase_seven_manual_verification_confirms_curated_project_flow_end_to_end(): void
    {
        Storage::fake('supabase');

        config()->set('filesystems.disks.supabase.endpoint', 'https://storage.example.test');
        config()->set('filesystems.disks.supabase.bucket', 'klass-storage-test');

        HomepageSection::create([
            'key' => 'project_recommendations',
            'label' => 'Project Recommendations',
            'position' => 1,
            'is_enabled' => true,
            'data_source' => 'recommended_projects',
        ]);

        $admin = User::factory()->admin()->create();

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->post(route('admin.recommended-projects.store'), [
                'title' => 'Manual Verification Curated Project',
                'description' => 'Curated project created during phase 7 end-to-end verification.',
                'ratio' => '16:9',
                'project_type' => 'web',
                'tags' => 'Laravel, Dashboard',
                'modules' => 'Auth, Reports',
                'thumbnail' => UploadedFile::fake()->image('manual-verification.png', 1280, 720),
                'display_priority' => 80,
                'is_active' => '1',
            ])
            ->assertRedirect(route('admin.homepage-sections.index'));

        $project = RecommendedProject::query()->latest('id')->firstOrFail();

        $this->assertNotNull($project->thumbnail_url);
        $this->assertCount(1, Storage::disk('supabase')->allFiles('gallery'));

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.total', 1)
            ->assertJsonPath('data.0.title', 'Manual Verification Curated Project');

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->put(route('admin.recommended-projects.update', $project), [
                'title' => 'Manual Verification Curated Project v2',
                'description' => 'Updated curated project during phase 7 verification.',
                'ratio' => '4:3',
                'project_type' => 'mobile',
                'tags' => 'Flutter, Analytics',
                'modules' => 'Feed, Insights',
                'display_priority' => 95,
            ])
            ->assertRedirect(route('admin.homepage-sections.index'));

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.total', 0)
            ->assertJsonCount(0, 'data');

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->patch(route('admin.recommended-projects.show-now', $project))
            ->assertRedirect(route('admin.homepage-sections.index'));

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.total', 1)
            ->assertJsonPath('data.0.title', 'Manual Verification Curated Project v2')
            ->assertJsonPath('data.0.project_type', 'mobile');

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->delete(route('admin.recommended-projects.destroy', $project))
            ->assertRedirect(route('admin.homepage-sections.index'));

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.total', 0)
            ->assertJsonCount(0, 'data');
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
            'recommendation_item_id' => $sourceType === RecommendedProject::SOURCE_SYSTEM_TOPIC
                ? 'system_topic_' . $sourceReference
                : $sourceReference,
            'source_type' => $sourceType,
            'source_reference' => $sourceReference,
            'subject_id' => $subjectId,
            'sub_subject_id' => $subSubjectId,
            'first_distributed_at' => $distributedAt,
            'last_distributed_at' => $distributedAt,
        ]);
    }
}