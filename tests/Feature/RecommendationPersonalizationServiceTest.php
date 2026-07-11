<?php

namespace Tests\Feature;

use App\Models\SubSubject;
use App\Models\Subject;
use App\Models\Topic;
use App\Models\User;
use App\Services\RecommendationPersonalizationService;
use Carbon\CarbonImmutable;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Tests\TestCase;

class RecommendationPersonalizationServiceTest extends TestCase
{
    use RefreshDatabase;

    public function test_service_ranks_authored_activity_by_primary_subject_then_frequency_and_recency(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new RecommendationPersonalizationService();
        $mathematics = Subject::query()->where('slug', 'mathematics')->firstOrFail();
        $algebra = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $geometry = SubSubject::query()->where('slug', 'geometry')->firstOrFail();
        $history = SubSubject::query()->where('slug', 'indonesian-history')->firstOrFail();

        $teacher = User::factory()->teacher()->create([
            'primary_subject_id' => $mathematics->id,
        ]);

        $geometryTopics = [
            Topic::create([
                'title' => 'Geometry Topic A',
                'teacher_id' => (string) $teacher->id,
                'sub_subject_id' => $geometry->id,
            ]),
            Topic::create([
                'title' => 'Geometry Topic B',
                'teacher_id' => (string) $teacher->id,
                'sub_subject_id' => $geometry->id,
            ]),
        ];

        $algebraTopics = [
            Topic::create([
                'title' => 'Algebra Topic A',
                'teacher_id' => (string) $teacher->id,
                'sub_subject_id' => $algebra->id,
            ]),
            Topic::create([
                'title' => 'Algebra Topic B',
                'teacher_id' => (string) $teacher->id,
                'sub_subject_id' => $algebra->id,
            ]),
        ];

        $historyTopics = [
            Topic::create([
                'title' => 'History Topic A',
                'teacher_id' => (string) $teacher->id,
                'sub_subject_id' => $history->id,
            ]),
            Topic::create([
                'title' => 'History Topic B',
                'teacher_id' => (string) $teacher->id,
                'sub_subject_id' => $history->id,
            ]),
            Topic::create([
                'title' => 'History Topic C',
                'teacher_id' => (string) $teacher->id,
                'sub_subject_id' => $history->id,
            ]),
        ];

        foreach ($geometryTopics as $topic) {
            DB::table('topics')->where('id', $topic->id)->update([
                'created_at' => CarbonImmutable::parse('2026-04-04 08:00:00'),
                'updated_at' => CarbonImmutable::parse('2026-04-04 08:00:00'),
            ]);
        }

        foreach ($algebraTopics as $topic) {
            DB::table('topics')->where('id', $topic->id)->update([
                'created_at' => CarbonImmutable::parse('2026-04-03 08:00:00'),
                'updated_at' => CarbonImmutable::parse('2026-04-03 08:00:00'),
            ]);
        }

        foreach ($historyTopics as $topic) {
            DB::table('topics')->where('id', $topic->id)->update([
                'created_at' => CarbonImmutable::parse('2026-04-05 08:00:00'),
                'updated_at' => CarbonImmutable::parse('2026-04-05 08:00:00'),
            ]);
        }

        $context = $service->resolve($teacher->fresh('primarySubject'));

        $this->assertTrue($context['public']['signals_available']);
        $this->assertTrue($context['public']['has_primary_subject']);
        $this->assertTrue($context['public']['has_authored_topic_activity']);
        $this->assertSame('profile_subject_with_authored_activity', $context['public']['signal_source']);
        $this->assertSame('mathematics', $context['public']['subject_anchor']['slug']);
        $this->assertSame(
            [$geometry->id, $algebra->id, $history->id],
            $context['public']['candidate_sub_subject_ids'],
        );
        $this->assertSame('geometry', $context['public']['candidate_sub_subjects'][0]['sub_subject']['slug']);
        $this->assertSame(2, $context['public']['candidate_sub_subjects'][0]['topic_count']);
        $this->assertSame('algebra', $context['public']['candidate_sub_subjects'][1]['sub_subject']['slug']);
        $this->assertSame('indonesian-history', $context['public']['candidate_sub_subjects'][2]['sub_subject']['slug']);
    }
}