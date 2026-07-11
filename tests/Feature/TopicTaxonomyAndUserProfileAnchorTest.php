<?php

namespace Tests\Feature;

use App\Models\SubSubject;
use App\Models\Subject;
use App\Models\Topic;
use App\Models\User;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class TopicTaxonomyAndUserProfileAnchorTest extends TestCase
{
    use RefreshDatabase;

    public function test_topic_api_returns_taxonomy_context_from_sub_subject_relation(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'quantum-physics')->firstOrFail();

        $topic = Topic::create([
            'title' => 'Quantum Mechanics Primer',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $subSubject->id,
            'thumbnail_url' => 'https://example.com/quantum.jpg',
        ]);

        $this->getJson('/api/v1/topics?search=Quantum')
            ->assertOk()
            ->assertJsonPath('meta.total', 1)
            ->assertJsonPath('data.0.id', $topic->id)
            ->assertJsonPath('data.0.sub_subject_id', $subSubject->id)
            ->assertJsonPath('data.0.subject_id', $subSubject->subject_id)
            ->assertJsonPath('data.0.taxonomy.subject.slug', 'science')
            ->assertJsonPath('data.0.taxonomy.sub_subject.slug', 'quantum-physics');

        $this->getJson('/api/v1/topics/' . $topic->id)
            ->assertOk()
            ->assertJsonPath('data.sub_subject_id', $subSubject->id)
            ->assertJsonPath('data.subject_id', $subSubject->subject_id)
            ->assertJsonPath('data.taxonomy.subject.name', 'Science')
            ->assertJsonPath('data.taxonomy.sub_subject.name', 'Quantum Physics');
    }

    public function test_user_profile_subject_is_optional_and_exposed_via_auth_me_resource(): void
    {
        $teacher = User::factory()->teacher()->create([
            'primary_subject_id' => null,
        ]);

        Sanctum::actingAs($teacher);

        $this->getJson('/api/v1/auth/me?include_personalization_context=1')
            ->assertOk()
            ->assertJsonPath('data.primary_subject_id', null)
            ->assertJsonPath('data.primary_subject', null)
            ->assertJsonPath('data.personalization_subject', null);
    }

    public function test_topic_api_marks_topics_without_sub_subject_as_general_feed_only_for_personalization(): void
    {
        $teacher = User::factory()->teacher()->create();

        $topic = Topic::create([
            'title' => 'Draft Topic Without Taxonomy',
            'teacher_id' => (string) $teacher->id,
        ]);

        $this->getJson('/api/v1/topics/' . $topic->id)
            ->assertOk()
            ->assertJsonPath('data.taxonomy', null)
            ->assertJsonPath('data.personalization.eligible', false)
            ->assertJsonPath('data.personalization.mode', Topic::PERSONALIZATION_MODE_GENERAL_FEED_ONLY)
            ->assertJsonPath('data.personalization.has_adequate_taxonomy', false)
            ->assertJsonPath('data.personalization.has_normalized_ownership', true)
            ->assertJsonPath('data.personalization.excluded_reason', Topic::PERSONALIZATION_EXCLUSION_MISSING_SUB_SUBJECT);
    }

    public function test_user_primary_subject_profile_has_priority_over_authored_topic_activity(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $science = Subject::query()->where('slug', 'science')->firstOrFail();
        $historySubSubject = SubSubject::query()->where('slug', 'indonesian-history')->firstOrFail();

        $teacher = User::factory()->teacher()->create([
            'primary_subject_id' => $science->id,
        ]);

        Topic::create([
            'title' => 'History Topic',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $historySubSubject->id,
        ]);

        $teacher->refresh();

        $this->assertSame($science->id, $teacher->resolvePersonalizationSubjectAnchor()?->id);
        $this->assertSame('profile', $teacher->resolvePersonalizationSubjectSource());
    }

    public function test_user_authored_topic_activity_becomes_fallback_anchor_when_primary_subject_is_missing(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $algebra = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $geometry = SubSubject::query()->where('slug', 'geometry')->firstOrFail();
        $history = SubSubject::query()->where('slug', 'indonesian-history')->firstOrFail();

        $teacher = User::factory()->teacher()->create([
            'primary_subject_id' => null,
        ]);

        Topic::create([
            'title' => 'Algebra Drill',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $algebra->id,
        ]);

        Topic::create([
            'title' => 'Geometry Drill',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $geometry->id,
        ]);

        Topic::create([
            'title' => 'History Drill',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $history->id,
        ]);

        $teacher->refresh();

        $resolvedSubject = $teacher->resolvePersonalizationSubjectAnchor();

        $this->assertNotNull($resolvedSubject);
        $this->assertSame('mathematics', $resolvedSubject?->slug);
        $this->assertSame('authored_topic_activity', $teacher->resolvePersonalizationSubjectSource());

        Sanctum::actingAs($teacher);

        $this->getJson('/api/v1/auth/me?include_personalization_context=1')
            ->assertOk()
            ->assertJsonPath('data.primary_subject_id', null)
            ->assertJsonPath('data.personalization_subject.slug', 'mathematics')
            ->assertJsonPath('data.personalization_subject.source', 'authored_topic_activity');
    }
}