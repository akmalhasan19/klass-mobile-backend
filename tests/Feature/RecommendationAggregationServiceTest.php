<?php

namespace Tests\Feature;

use App\Http\Resources\RecommendedProjectRecommendationCollection;
use App\Models\Content;
use App\Models\RecommendedProject;
use App\Models\SubSubject;
use App\Models\SystemRecommendationAssignment;
use App\Models\Topic;
use App\Models\User;
use App\Services\RecommendationAggregationService;
use App\Services\RecommendationPersonalizationService;
use Carbon\CarbonImmutable;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Tests\TestCase;

class RecommendationAggregationServiceTest extends TestCase
{
    use RefreshDatabase;

    public function test_service_merges_curated_and_topic_sources_into_mobile_payload(): void
    {
        $service = new RecommendationAggregationService();

        $baseTime = CarbonImmutable::parse('2026-04-03 10:00:00');

        $adminProject = RecommendedProject::factory()->create([
            'title' => 'Admin Showcase',
            'display_priority' => 100,
            'source_type' => RecommendedProject::SOURCE_ADMIN_UPLOAD,
            'source_payload' => ['score' => 1.5],
        ]);

        $curatedSystemTopic = RecommendedProject::factory()->create([
            'title' => 'Curated React Topic',
            'description' => 'Override dari source topic lama.',
            'thumbnail_url' => 'https://example.com/react-override.jpg',
            'project_type' => 'web',
            'tags' => ['React'],
            'modules' => ['Hooks', 'State'],
            'source_type' => RecommendedProject::SOURCE_SYSTEM_TOPIC,
            'source_reference' => 'topic-react',
            'source_payload' => ['score' => 4.2],
            'display_priority' => 20,
        ]);

        $topicFromSystem = Topic::unguarded(fn () => Topic::create([
            'id' => 'topic-flutter',
            'title' => 'Belajar Flutter Dasar',
            'teacher_id' => 'teacher-1',
            'thumbnail_url' => 'https://example.com/flutter.jpg',
            'is_published' => true,
            'order' => 1,
        ]));
        DB::table('topics')->where('id', $topicFromSystem->id)->update([
            'created_at' => $baseTime->subDay(),
            'updated_at' => $baseTime->subDay(),
        ]);
        $topicFromSystem->refresh();

        Content::create([
            'topic_id' => $topicFromSystem->id,
            'type' => 'module',
            'title' => 'Routing',
            'data' => ['kind' => 'module'],
            'media_url' => null,
            'is_published' => true,
            'order' => 1,
        ]);

        Content::create([
            'topic_id' => $topicFromSystem->id,
            'type' => 'module',
            'title' => 'State Management',
            'data' => ['kind' => 'module'],
            'media_url' => null,
            'is_published' => true,
            'order' => 2,
        ]);

        $suppressedTopic = Topic::unguarded(fn () => Topic::create([
            'id' => 'topic-react',
            'title' => 'React Lama',
            'teacher_id' => 'teacher-2',
            'thumbnail_url' => 'https://example.com/react.jpg',
            'is_published' => true,
            'order' => 2,
        ]));
        DB::table('topics')->where('id', $suppressedTopic->id)->update([
            'created_at' => $baseTime->subHours(6),
            'updated_at' => $baseTime->subHours(6),
        ]);
        $suppressedTopic->refresh();

        $feed = $service->buildFeed($baseTime);
        $payload = (new RecommendedProjectRecommendationCollection($feed))->response()->getData(true);

        $this->assertCount(3, $feed);
        $this->assertSame((string) $adminProject->id, $feed[0]['id']);
        $this->assertSame((string) $curatedSystemTopic->id, $feed[1]['id']);
        $this->assertSame('system_topic_topic-flutter', $feed[2]['id']);
        $this->assertSame(['Routing', 'State Management'], $feed[2]['modules']);
        $this->assertSame([], $feed[2]['tags']);

        $this->assertSame(3, $payload['meta']['total']);
        $this->assertSame(1, $payload['meta']['source_breakdown'][RecommendedProject::SOURCE_ADMIN_UPLOAD]);
        $this->assertSame(2, $payload['meta']['source_breakdown'][RecommendedProject::SOURCE_SYSTEM_TOPIC]);
        $this->assertArrayNotHasKey('source_payload', $payload['data'][0]);
        $this->assertArrayNotHasKey('source_reference', $payload['data'][0]);
        $this->assertSame('Belajar Flutter Dasar', $payload['data'][2]['title']);
        $this->assertSame(['Routing', 'State Management'], $payload['data'][2]['modules']);
    }

    public function test_service_sorts_by_priority_then_score_then_created_at(): void
    {
        $service = new RecommendationAggregationService();

        $first = RecommendedProject::factory()->create([
            'title' => 'Lower Score',
            'display_priority' => 50,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => ['score' => 2.1],
        ]);

        $second = RecommendedProject::factory()->create([
            'title' => 'Higher Score',
            'display_priority' => 50,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => ['score' => 9.4],
        ]);

        $third = RecommendedProject::factory()->create([
            'title' => 'Same Score Newer',
            'display_priority' => 50,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => ['score' => 9.4],
        ]);

        DB::table('recommended_projects')->where('id', $first->id)->update([
            'created_at' => CarbonImmutable::parse('2026-04-03 08:00:00'),
            'updated_at' => CarbonImmutable::parse('2026-04-03 08:00:00'),
        ]);
        DB::table('recommended_projects')->where('id', $second->id)->update([
            'created_at' => CarbonImmutable::parse('2026-04-03 09:00:00'),
            'updated_at' => CarbonImmutable::parse('2026-04-03 09:00:00'),
        ]);
        DB::table('recommended_projects')->where('id', $third->id)->update([
            'created_at' => CarbonImmutable::parse('2026-04-03 10:00:00'),
            'updated_at' => CarbonImmutable::parse('2026-04-03 10:00:00'),
        ]);

        $feed = $service->buildFeed(CarbonImmutable::parse('2026-04-03 12:00:00'));

        $this->assertSame('Same Score Newer', $feed[0]['title']);
        $this->assertSame('Higher Score', $feed[1]['title']);
        $this->assertSame('Lower Score', $feed[2]['title']);
    }

    public function test_service_filters_visibility_and_handles_empty_sources_safely(): void
    {
        $service = new RecommendationAggregationService();
        $moment = CarbonImmutable::parse('2026-04-03 12:00:00');

        RecommendedProject::factory()->inactive()->create([
            'title' => 'Inactive Item',
        ]);

        RecommendedProject::factory()->scheduled()->create([
            'title' => 'Scheduled Item',
            'starts_at' => $moment->addDay(),
        ]);

        RecommendedProject::factory()->expired()->create([
            'title' => 'Expired Item',
            'ends_at' => $moment->subDay(),
        ]);

        $feed = $service->buildFeed($moment);

        $this->assertCount(0, $feed);

        Topic::unguarded(fn () => Topic::create([
            'id' => 'topic-hidden',
            'title' => 'Topic Hidden by Override',
            'teacher_id' => 'teacher-9',
            'thumbnail_url' => null,
            'is_published' => true,
            'order' => 1,
        ]));

        RecommendedProject::factory()->inactive()->create([
            'title' => 'Inactive Topic Override',
            'source_type' => RecommendedProject::SOURCE_SYSTEM_TOPIC,
            'source_reference' => 'topic-hidden',
        ]);

        $feedAfterOverride = $service->buildFeed($moment);

        $this->assertCount(0, $feedAfterOverride);
    }

    public function test_service_keeps_topics_visible_in_general_feed_when_guardrails_exclude_them_from_personalization(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new RecommendationAggregationService();
        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $missingTaxonomyTopic = Topic::create([
            'title' => 'Missing Taxonomy Feed Topic',
            'teacher_id' => (string) $teacher->id,
            'thumbnail_url' => 'https://example.com/missing-taxonomy.jpg',
        ]);

        $unresolvedOwnershipTopic = Topic::create([
            'title' => 'Unresolved Ownership Feed Topic',
            'teacher_id' => 'legacy-owner-reference',
            'sub_subject_id' => $subSubject->id,
            'thumbnail_url' => 'https://example.com/unresolved-owner.jpg',
        ]);

        $feed = $service->buildFeed();

        $missingTaxonomyItem = $feed->firstWhere('id', 'system_topic_' . $missingTaxonomyTopic->id);
        $unresolvedOwnershipItem = $feed->firstWhere('id', 'system_topic_' . $unresolvedOwnershipTopic->id);

        $this->assertNotNull($missingTaxonomyItem);
        $this->assertNotNull($unresolvedOwnershipItem);
        $this->assertFalse($missingTaxonomyItem['personalization']['eligible']);
        $this->assertSame(Topic::PERSONALIZATION_MODE_GENERAL_FEED_ONLY, $missingTaxonomyItem['personalization']['mode']);
        $this->assertSame(Topic::PERSONALIZATION_EXCLUSION_MISSING_SUB_SUBJECT, $missingTaxonomyItem['personalization']['excluded_reason']);
        $this->assertTrue($missingTaxonomyItem['personalization']['has_normalized_ownership']);

        $this->assertFalse($unresolvedOwnershipItem['personalization']['eligible']);
        $this->assertSame(Topic::PERSONALIZATION_MODE_GENERAL_FEED_ONLY, $unresolvedOwnershipItem['personalization']['mode']);
        $this->assertSame(Topic::PERSONALIZATION_EXCLUSION_UNRESOLVED_OWNERSHIP, $unresolvedOwnershipItem['personalization']['excluded_reason']);
        $this->assertTrue($unresolvedOwnershipItem['personalization']['has_adequate_taxonomy']);
    }

    public function test_service_filters_system_generated_candidates_for_authenticated_user_without_displacing_admin_curated_items(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new RecommendationAggregationService();
        $personalizationService = new RecommendationPersonalizationService();
        $science = \App\Models\Subject::query()->where('slug', 'science')->firstOrFail();
        $quantumPhysics = SubSubject::query()->where('slug', 'quantum-physics')->firstOrFail();
        $thermodynamics = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();
        $algebra = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $history = SubSubject::query()->where('slug', 'indonesian-history')->firstOrFail();

        $teacher = User::factory()->teacher()->create([
            'primary_subject_id' => $science->id,
        ]);
        $otherTeacher = User::factory()->teacher()->create();

        RecommendedProject::factory()->create([
            'title' => 'Admin Showcase',
            'display_priority' => 100,
            'source_type' => RecommendedProject::SOURCE_ADMIN_UPLOAD,
        ]);

        Topic::create([
            'title' => 'Quantum Activity Topic',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $quantumPhysics->id,
        ]);

        Topic::create([
            'title' => 'Algebra Activity Topic',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $algebra->id,
        ]);

        RecommendedProject::factory()->create([
            'title' => 'Thermodynamics AI Candidate',
            'display_priority' => 30,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'score' => 6.7,
                'sub_subject_id' => $thermodynamics->id,
                'subject_id' => $science->id,
                'taxonomy' => [
                    'subject' => [
                        'id' => $science->id,
                        'name' => $science->name,
                        'slug' => $science->slug,
                    ],
                    'sub_subject' => [
                        'id' => $thermodynamics->id,
                        'subject_id' => $science->id,
                        'name' => $thermodynamics->name,
                        'slug' => $thermodynamics->slug,
                    ],
                ],
            ],
        ]);

        RecommendedProject::factory()->create([
            'title' => 'History AI Candidate',
            'display_priority' => 80,
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'score' => 9.9,
                'sub_subject_id' => $history->id,
                'subject_id' => $history->subject_id,
            ],
        ]);

        Topic::create([
            'title' => 'History General Topic',
            'teacher_id' => (string) $otherTeacher->id,
            'sub_subject_id' => $history->id,
        ]);

        $context = $personalizationService->resolve($teacher->fresh('primarySubject'));
        $snapshot = $service->buildFeedSnapshot(personalizationContext: $context);
        $titles = $snapshot['items']->pluck('title')->take(5)->all();

        $this->assertSame([
            'Admin Showcase',
            'Quantum Activity Topic',
            'Thermodynamics AI Candidate',
            'Algebra Activity Topic',
        ], $titles);
        $this->assertTrue($snapshot['personalization']['applied']);
        $this->assertTrue($snapshot['personalization']['filter_applied']);
        $this->assertSame(2, $snapshot['personalization']['matched_system_topic_count']);
        $this->assertSame(3, $snapshot['personalization']['selected_system_candidate_count']);
        $this->assertSame(2, $snapshot['personalization']['filtered_out_system_candidate_count']);
        $this->assertSame(1, $snapshot['personalization']['selected_source_breakdown'][RecommendedProject::SOURCE_AI_GENERATED]);
        $this->assertSame(2, $snapshot['personalization']['selected_source_breakdown'][RecommendedProject::SOURCE_SYSTEM_TOPIC]);
        $this->assertSame([$quantumPhysics->id, $thermodynamics->id, $algebra->id], $snapshot['personalization']['matched_sub_subject_ids']);
        $this->assertNotContains('History AI Candidate', $snapshot['items']->pluck('title')->all());
        $this->assertNotContains('History General Topic', $snapshot['items']->pluck('title')->all());
    }

    public function test_service_keeps_persisted_system_topic_override_when_candidate_filter_suppresses_raw_duplicate(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new RecommendationAggregationService();
        $personalizationService = new RecommendationPersonalizationService();
        $mathematics = \App\Models\Subject::query()->where('slug', 'mathematics')->firstOrFail();
        $algebra = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $teacher = User::factory()->teacher()->create([
            'primary_subject_id' => $mathematics->id,
        ]);

        $sourceTopic = Topic::unguarded(fn () => Topic::create([
            'id' => 'topic-algebra-source',
            'title' => 'Raw Algebra Topic',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $algebra->id,
            'thumbnail_url' => 'https://example.com/raw-algebra.jpg',
            'is_published' => true,
            'order' => 1,
        ]));

        RecommendedProject::factory()->create([
            'title' => 'Persisted Algebra Override',
            'display_priority' => 20,
            'source_type' => RecommendedProject::SOURCE_SYSTEM_TOPIC,
            'source_reference' => $sourceTopic->id,
            'source_payload' => [
                'score' => 7.2,
            ],
        ]);

        $context = $personalizationService->resolve($teacher->fresh('primarySubject'));
        $snapshot = $service->buildFeedSnapshot(personalizationContext: $context);

        $this->assertContains('Persisted Algebra Override', $snapshot['items']->pluck('title')->all());
        $this->assertNotContains('Raw Algebra Topic', $snapshot['items']->pluck('title')->all());
        $this->assertTrue($snapshot['personalization']['applied']);
        $this->assertSame(1, $snapshot['personalization']['matched_system_topic_count']);
    }

    public function test_service_builds_summary_with_one_top_item_per_sub_subject(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new RecommendationAggregationService();
        $science = \App\Models\Subject::query()->where('slug', 'science')->firstOrFail();
        $mathematics = \App\Models\Subject::query()->where('slug', 'mathematics')->firstOrFail();
        $thermodynamics = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();
        $algebra = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $thermoOlder = RecommendedProject::factory()->create([
            'title' => 'Thermodynamics Older Candidate',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $science->id,
                'sub_subject_id' => $thermodynamics->id,
            ],
        ]);
        $thermoWinner = RecommendedProject::factory()->create([
            'title' => 'Thermodynamics Winner Candidate',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $science->id,
                'sub_subject_id' => $thermodynamics->id,
            ],
        ]);

        $algebraWinner = Topic::unguarded(fn () => Topic::create([
            'id' => 'topic-algebra-winner',
            'title' => 'Algebra Winner Topic',
            'teacher_id' => 'teacher-algebra-winner',
            'sub_subject_id' => $algebra->id,
            'is_published' => true,
            'order' => 1,
        ]));
        $algebraLoser = Topic::unguarded(fn () => Topic::create([
            'id' => 'topic-algebra-loser',
            'title' => 'Algebra Loser Topic',
            'teacher_id' => 'teacher-algebra-loser',
            'sub_subject_id' => $algebra->id,
            'is_published' => true,
            'order' => 2,
        ]));

        foreach (range(1, 2) as $index) {
            $user = User::factory()->create();

            $this->createSystemRecommendationAssignment(
                $user,
                RecommendedProject::SOURCE_AI_GENERATED,
                (string) $thermoOlder->id,
                $science->id,
                $thermodynamics->id,
                CarbonImmutable::parse('2026-04-07 09:00:00'),
            );
        }

        foreach (range(1, 2) as $index) {
            $user = User::factory()->create();

            $this->createSystemRecommendationAssignment(
                $user,
                RecommendedProject::SOURCE_AI_GENERATED,
                (string) $thermoWinner->id,
                $science->id,
                $thermodynamics->id,
                CarbonImmutable::parse('2026-04-07 10:00:00'),
            );
        }

        foreach (range(1, 3) as $index) {
            $user = User::factory()->create();

            $this->createSystemRecommendationAssignment(
                $user,
                RecommendedProject::SOURCE_SYSTEM_TOPIC,
                $algebraWinner->id,
                $mathematics->id,
                $algebra->id,
                CarbonImmutable::parse('2026-04-07 11:00:00'),
            );
        }

        foreach (range(1, 2) as $index) {
            $user = User::factory()->create();

            $this->createSystemRecommendationAssignment(
                $user,
                RecommendedProject::SOURCE_SYSTEM_TOPIC,
                $algebraLoser->id,
                $mathematics->id,
                $algebra->id,
                CarbonImmutable::parse('2026-04-07 12:00:00'),
            );
        }

        $summary = $service->buildSystemDistributionSummary();

        $this->assertCount(2, $summary);
        $this->assertSame($summary->count(), $summary->pluck('sub_subject_id')->unique()->count());

        $algebraSummary = $summary->firstWhere('sub_subject_id', $algebra->id);
        $thermodynamicsSummary = $summary->firstWhere('sub_subject_id', $thermodynamics->id);

        $this->assertNotNull($algebraSummary);
        $this->assertNotNull($thermodynamicsSummary);
        $this->assertSame('Algebra Winner Topic', $algebraSummary['title']);
        $this->assertSame(3, $algebraSummary['distinct_user_count']);
        $this->assertSame(RecommendedProject::SOURCE_SYSTEM_TOPIC, $algebraSummary['source_type']);
        $this->assertSame('Thermodynamics Winner Candidate', $thermodynamicsSummary['title']);
        $this->assertSame(2, $thermodynamicsSummary['distinct_user_count']);
        $this->assertSame(RecommendedProject::SOURCE_AI_GENERATED, $thermodynamicsSummary['source_type']);
        $this->assertSame('mathematics', data_get($algebraSummary, 'subject.slug'));
        $this->assertSame('science', data_get($thermodynamicsSummary, 'subject.slug'));
    }

    public function test_service_excludes_summary_candidates_below_minimum_distinct_user_threshold(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new RecommendationAggregationService();
        $mathematics = \App\Models\Subject::query()->where('slug', 'mathematics')->firstOrFail();
        $geometry = SubSubject::query()->where('slug', 'geometry')->firstOrFail();
        $arithmetic = SubSubject::query()->where('slug', 'arithmetic')->firstOrFail();

        $belowThreshold = RecommendedProject::factory()->create([
            'title' => 'Geometry Single Distribution',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $mathematics->id,
                'sub_subject_id' => $geometry->id,
            ],
        ]);
        $eligible = RecommendedProject::factory()->create([
            'title' => 'Arithmetic Eligible Distribution',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $mathematics->id,
                'sub_subject_id' => $arithmetic->id,
            ],
        ]);

        $this->createSystemRecommendationAssignment(
            User::factory()->create(),
            RecommendedProject::SOURCE_AI_GENERATED,
            (string) $belowThreshold->id,
            $mathematics->id,
            $geometry->id,
            CarbonImmutable::parse('2026-04-07 08:00:00'),
        );

        foreach (range(1, 2) as $index) {
            $this->createSystemRecommendationAssignment(
                User::factory()->create(),
                RecommendedProject::SOURCE_AI_GENERATED,
                (string) $eligible->id,
                $mathematics->id,
                $arithmetic->id,
                CarbonImmutable::parse('2026-04-07 09:00:00'),
            );
        }

        $summary = $service->buildSystemDistributionSummary();

        $this->assertCount(1, $summary);
        $this->assertSame('Arithmetic Eligible Distribution', $summary[0]['title']);
        $this->assertNotContains($geometry->id, $summary->pluck('sub_subject_id')->all());
    }

    public function test_service_uses_source_created_at_to_break_remaining_summary_ties(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new RecommendationAggregationService();
        $mathematics = \App\Models\Subject::query()->where('slug', 'mathematics')->firstOrFail();
        $geometry = SubSubject::query()->where('slug', 'geometry')->firstOrFail();

        $olderSource = RecommendedProject::factory()->create([
            'title' => 'Geometry Older Source',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $mathematics->id,
                'sub_subject_id' => $geometry->id,
            ],
        ]);
        $newerSource = RecommendedProject::factory()->create([
            'title' => 'Geometry Newer Source',
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_payload' => [
                'subject_id' => $mathematics->id,
                'sub_subject_id' => $geometry->id,
            ],
        ]);

        DB::table('recommended_projects')->where('id', $olderSource->id)->update([
            'created_at' => CarbonImmutable::parse('2026-04-07 07:00:00'),
            'updated_at' => CarbonImmutable::parse('2026-04-07 07:00:00'),
        ]);
        DB::table('recommended_projects')->where('id', $newerSource->id)->update([
            'created_at' => CarbonImmutable::parse('2026-04-07 08:00:00'),
            'updated_at' => CarbonImmutable::parse('2026-04-07 08:00:00'),
        ]);

        foreach (range(1, 2) as $index) {
            $user = User::factory()->create();

            $this->createSystemRecommendationAssignment(
                $user,
                RecommendedProject::SOURCE_AI_GENERATED,
                (string) $olderSource->id,
                $mathematics->id,
                $geometry->id,
                CarbonImmutable::parse('2026-04-07 10:00:00'),
            );
        }

        foreach (range(1, 2) as $index) {
            $user = User::factory()->create();

            $this->createSystemRecommendationAssignment(
                $user,
                RecommendedProject::SOURCE_AI_GENERATED,
                (string) $newerSource->id,
                $mathematics->id,
                $geometry->id,
                CarbonImmutable::parse('2026-04-07 10:00:00'),
            );
        }

        $summary = $service->buildSystemDistributionSummary();

        $this->assertCount(1, $summary);
        $this->assertSame('Geometry Newer Source', $summary[0]['title']);
    }

    public function test_service_falls_back_to_source_reference_for_stable_summary_ties(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new RecommendationAggregationService();
        $science = \App\Models\Subject::query()->where('slug', 'science')->firstOrFail();
        $physics = SubSubject::query()->where('slug', 'physics')->firstOrFail();

        $topicAlpha = Topic::unguarded(fn () => Topic::create([
            'id' => 'topic-alpha',
            'title' => 'Physics Alpha Topic',
            'teacher_id' => 'teacher-alpha',
            'sub_subject_id' => $physics->id,
            'is_published' => true,
            'order' => 1,
        ]));
        $topicBeta = Topic::unguarded(fn () => Topic::create([
            'id' => 'topic-beta',
            'title' => 'Physics Beta Topic',
            'teacher_id' => 'teacher-beta',
            'sub_subject_id' => $physics->id,
            'is_published' => true,
            'order' => 2,
        ]));

        DB::table('topics')->whereIn('id', [$topicAlpha->id, $topicBeta->id])->update([
            'created_at' => CarbonImmutable::parse('2026-04-07 06:00:00'),
            'updated_at' => CarbonImmutable::parse('2026-04-07 06:00:00'),
        ]);

        foreach (range(1, 2) as $index) {
            $this->createSystemRecommendationAssignment(
                User::factory()->create(),
                RecommendedProject::SOURCE_SYSTEM_TOPIC,
                $topicAlpha->id,
                $science->id,
                $physics->id,
                CarbonImmutable::parse('2026-04-07 11:00:00'),
            );
            $this->createSystemRecommendationAssignment(
                User::factory()->create(),
                RecommendedProject::SOURCE_SYSTEM_TOPIC,
                $topicBeta->id,
                $science->id,
                $physics->id,
                CarbonImmutable::parse('2026-04-07 11:00:00'),
            );
        }

        $firstSummary = $service->buildSystemDistributionSummary();
        $secondSummary = $service->buildSystemDistributionSummary();

        $this->assertCount(1, $firstSummary);
        $this->assertSame('topic-alpha', $firstSummary[0]['source_reference']);
        $this->assertSame('Physics Alpha Topic', $firstSummary[0]['title']);
        $this->assertEquals($firstSummary->toArray(), $secondSummary->toArray());
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