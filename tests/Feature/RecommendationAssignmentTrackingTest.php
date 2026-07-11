<?php

namespace Tests\Feature;

use App\Models\HomepageSection;
use App\Models\RecommendedProject;
use App\Models\SubSubject;
use App\Models\SystemRecommendationAssignment;
use App\Models\Topic;
use App\Models\User;
use App\Services\SystemRecommendationAssignmentService;
use Carbon\CarbonImmutable;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class RecommendationAssignmentTrackingTest extends TestCase
{
    use RefreshDatabase;

    public function test_authenticated_homepage_requests_upsert_system_recommendation_assignments_without_duplicate_distinct_counts(): void
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
        $thermodynamics = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();
        $viewer = User::factory()->teacher()->create();
        $topicOwner = User::factory()->teacher()->create();

        RecommendedProject::factory()->create([
            'title' => 'Admin Showcase',
            'display_priority' => 100,
            'source_type' => RecommendedProject::SOURCE_ADMIN_UPLOAD,
        ]);

        $aiRecommendation = RecommendedProject::factory()->create([
            'title' => 'Thermodynamics AI Candidate',
            'display_priority' => 40,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_reference' => null,
            'source_payload' => [
                'score' => 7.5,
                'subject_id' => $science->id,
                'sub_subject_id' => $thermodynamics->id,
            ],
        ]);

        $topic = Topic::create([
            'title' => 'Thermodynamics Topic',
            'teacher_id' => (string) $topicOwner->id,
            'sub_subject_id' => $thermodynamics->id,
            'is_published' => true,
            'order' => 1,
        ]);

        Sanctum::actingAs($viewer);

        $firstMoment = CarbonImmutable::parse('2026-04-07 09:00:00');
        $secondMoment = CarbonImmutable::parse('2026-04-07 09:15:00');

        $this->travelTo($firstMoment);

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.personalization.policy_version', 'phase_4_2_assignment_tracking_deduplication')
            ->assertJsonPath('meta.personalization.tracks_assignments', true)
            ->assertJsonPath('data.0.title', 'Admin Showcase');

        $this->assertDatabaseCount('system_recommendation_assignments', 2);
        $this->assertDatabaseHas('system_recommendation_assignments', [
            'user_id' => $viewer->id,
            'recommendation_key' => RecommendedProject::SOURCE_AI_GENERATED . ':' . $aiRecommendation->id,
            'recommendation_item_id' => (string) $aiRecommendation->id,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_reference' => (string) $aiRecommendation->id,
            'subject_id' => $science->id,
            'sub_subject_id' => $thermodynamics->id,
        ]);
        $this->assertDatabaseHas('system_recommendation_assignments', [
            'user_id' => $viewer->id,
            'recommendation_key' => RecommendedProject::SOURCE_SYSTEM_TOPIC . ':' . $topic->id,
            'recommendation_item_id' => 'system_topic_' . $topic->id,
            'source_type' => RecommendedProject::SOURCE_SYSTEM_TOPIC,
            'source_reference' => $topic->id,
            'subject_id' => $science->id,
            'sub_subject_id' => $thermodynamics->id,
        ]);

        $initialAiAssignment = SystemRecommendationAssignment::query()
            ->where('user_id', $viewer->id)
            ->where('recommendation_key', RecommendedProject::SOURCE_AI_GENERATED . ':' . $aiRecommendation->id)
            ->firstOrFail();
        $initialTopicAssignment = SystemRecommendationAssignment::query()
            ->where('user_id', $viewer->id)
            ->where('recommendation_key', RecommendedProject::SOURCE_SYSTEM_TOPIC . ':' . $topic->id)
            ->firstOrFail();

        $this->assertTrue($initialAiAssignment->first_distributed_at->equalTo($firstMoment));
        $this->assertTrue($initialAiAssignment->last_distributed_at->equalTo($firstMoment));
        $this->assertTrue($initialTopicAssignment->first_distributed_at->equalTo($firstMoment));
        $this->assertTrue($initialTopicAssignment->last_distributed_at->equalTo($firstMoment));

        $this->travelTo($secondMoment);

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.personalization.tracks_assignments', true);

        $this->assertDatabaseCount('system_recommendation_assignments', 2);

        $updatedAiAssignment = $initialAiAssignment->fresh();
        $updatedTopicAssignment = $initialTopicAssignment->fresh();

        $this->assertTrue($updatedAiAssignment->first_distributed_at->equalTo($firstMoment));
        $this->assertTrue($updatedAiAssignment->last_distributed_at->equalTo($secondMoment));
        $this->assertTrue($updatedTopicAssignment->first_distributed_at->equalTo($firstMoment));
        $this->assertTrue($updatedTopicAssignment->last_distributed_at->equalTo($secondMoment));
    }

    public function test_guest_homepage_requests_do_not_track_system_recommendation_assignments(): void
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
        $thermodynamics = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();

        RecommendedProject::factory()->create([
            'title' => 'Guest Safe AI Candidate',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $science->id,
                'sub_subject_id' => $thermodynamics->id,
            ],
        ]);

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.personalization.audience', 'guest')
            ->assertJsonPath('meta.personalization.tracks_assignments', false);

        $this->assertDatabaseCount('system_recommendation_assignments', 0);
    }

    public function test_tracking_failures_do_not_break_homepage_response(): void
    {
        HomepageSection::create([
            'key' => 'project_recommendations',
            'label' => 'Project Recommendations',
            'position' => 1,
            'is_enabled' => true,
            'data_source' => 'recommended_projects',
        ]);

        $viewer = User::factory()->teacher()->create();

        RecommendedProject::factory()->create([
            'title' => 'Admin Showcase',
            'display_priority' => 100,
            'source_type' => RecommendedProject::SOURCE_ADMIN_UPLOAD,
        ]);

        $this->mock(SystemRecommendationAssignmentService::class, function ($mock): void {
            $mock->shouldReceive('trackServedRecommendations')
                ->once()
                ->andThrow(new \RuntimeException('Tracking failed.'));
        });

        Sanctum::actingAs($viewer);

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('meta.personalization.policy_version', 'phase_4_2_assignment_tracking_deduplication')
            ->assertJsonPath('data.0.title', 'Admin Showcase');

        $this->assertDatabaseCount('system_recommendation_assignments', 0);
    }
}