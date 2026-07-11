<?php

namespace Tests\Feature;

use App\Models\SubSubject;
use App\Models\Subject;
use App\Models\Topic;
use App\Models\User;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Tests\TestCase;

class SubjectTaxonomyAndTopicOwnershipTest extends TestCase
{
    use RefreshDatabase;

    public function test_subject_taxonomy_seeder_creates_baseline_subjects_and_sub_subjects(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $this->assertGreaterThan(5, Subject::query()->count());
        $this->assertGreaterThan(15, SubSubject::query()->count());

        $mathematics = Subject::query()
            ->where('slug', 'mathematics')
            ->with('subSubjects')
            ->firstOrFail();

        $this->assertSame('Mathematics', $mathematics->name);
        $this->assertSame(
            ['Algebra', 'Geometry', 'Arithmetic'],
            $mathematics->subSubjects->pluck('name')->all(),
        );
        $this->assertTrue(Subject::query()->where('slug', 'ipas-sd')->exists());
        $this->assertTrue(SubSubject::query()->where('slug', 'gaya-sekitar-kita-kelas-4')->exists());
    }

    public function test_sub_subject_factory_creates_a_related_subject(): void
    {
        $subSubject = SubSubject::factory()->create();

        $this->assertInstanceOf(Subject::class, $subSubject->subject);
        $this->assertNotNull($subSubject->subject->id);
    }

    public function test_topic_model_auto_normalizes_ownership_when_teacher_identifier_matches_a_user(): void
    {
        $teacher = User::factory()->teacher()->create([
            'email' => 'teacher.normalized@example.com',
        ]);

        $topic = Topic::create([
            'title' => 'Normalized Topic',
            'teacher_id' => $teacher->email,
        ]);

        $this->assertSame($teacher->id, $topic->owner_user_id);
        $this->assertSame(Topic::OWNERSHIP_STATUS_NORMALIZED, $topic->ownership_status);
        $this->assertTrue($topic->hasNormalizedOwnership());
    }

    public function test_topic_ownership_backfill_maps_numeric_and_email_teacher_ids_and_excludes_unresolved_topics_from_personalization_scope(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $numericTeacher = User::factory()->teacher()->create();
        $emailTeacher = User::factory()->teacher()->create([
            'email' => 'topic-owner@example.com',
        ]);
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        DB::table('topics')->insert([
            [
                'id' => (string) str()->uuid(),
                'title' => 'Numeric Legacy Topic',
                'teacher_id' => (string) $numericTeacher->id,
                'sub_subject_id' => $subSubject->id,
                'thumbnail_url' => null,
                'is_published' => true,
                'order' => 0,
                'owner_user_id' => null,
                'ownership_status' => Topic::OWNERSHIP_STATUS_LEGACY_UNRESOLVED,
                'created_at' => now(),
                'updated_at' => now(),
            ],
            [
                'id' => (string) str()->uuid(),
                'title' => 'Email Legacy Topic',
                'teacher_id' => strtoupper($emailTeacher->email),
                'sub_subject_id' => $subSubject->id,
                'thumbnail_url' => null,
                'is_published' => true,
                'order' => 0,
                'owner_user_id' => null,
                'ownership_status' => Topic::OWNERSHIP_STATUS_LEGACY_UNRESOLVED,
                'created_at' => now(),
                'updated_at' => now(),
            ],
            [
                'id' => (string) str()->uuid(),
                'title' => 'Unresolved Legacy Topic',
                'teacher_id' => 'legacy-teacher-reference',
                'sub_subject_id' => $subSubject->id,
                'thumbnail_url' => null,
                'is_published' => true,
                'order' => 0,
                'owner_user_id' => null,
                'ownership_status' => Topic::OWNERSHIP_STATUS_LEGACY_UNRESOLVED,
                'created_at' => now(),
                'updated_at' => now(),
            ],
        ]);

        $this->artisan('app:backfill-topic-ownership')
            ->expectsOutputToContain('Processed 3 topics. Normalized: 2. Legacy unresolved: 1.')
            ->assertExitCode(0);

        $numericTopic = Topic::query()->where('title', 'Numeric Legacy Topic')->firstOrFail();
        $emailTopic = Topic::query()->where('title', 'Email Legacy Topic')->firstOrFail();
        $unresolvedTopic = Topic::query()->where('title', 'Unresolved Legacy Topic')->firstOrFail();

        $this->assertSame($numericTeacher->id, $numericTopic->owner_user_id);
        $this->assertSame(Topic::OWNERSHIP_STATUS_NORMALIZED, $numericTopic->ownership_status);

        $this->assertSame($emailTeacher->id, $emailTopic->owner_user_id);
        $this->assertSame(Topic::OWNERSHIP_STATUS_NORMALIZED, $emailTopic->ownership_status);

        $this->assertNull($unresolvedTopic->owner_user_id);
        $this->assertSame(Topic::OWNERSHIP_STATUS_LEGACY_UNRESOLVED, $unresolvedTopic->ownership_status);

        $this->assertSame(
            ['Email Legacy Topic', 'Numeric Legacy Topic'],
            Topic::query()
                ->eligibleForPersonalization()
                ->orderBy('title')
                ->pluck('title')
                ->all(),
        );
    }

    public function test_personalization_scope_requires_both_normalized_ownership_and_sub_subject_taxonomy(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $eligibleTopic = Topic::create([
            'title' => 'Eligible Topic',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $subSubject->id,
        ]);

        $missingTaxonomyTopic = Topic::create([
            'title' => 'Missing Taxonomy Topic',
            'teacher_id' => (string) $teacher->id,
        ]);

        $unresolvedOwnershipTopic = Topic::create([
            'title' => 'Unresolved Ownership Topic',
            'teacher_id' => 'legacy-owner-ref',
            'sub_subject_id' => $subSubject->id,
        ]);

        $this->assertSame(
            ['Eligible Topic'],
            Topic::query()
                ->eligibleForPersonalization()
                ->orderBy('title')
                ->pluck('title')
                ->all(),
        );

        $this->assertTrue($eligibleTopic->isEligibleForPersonalization());
        $this->assertFalse($missingTaxonomyTopic->isEligibleForPersonalization());
        $this->assertFalse($unresolvedOwnershipTopic->isEligibleForPersonalization());
        $this->assertSame(
            Topic::PERSONALIZATION_EXCLUSION_MISSING_SUB_SUBJECT,
            $missingTaxonomyTopic->resolvePersonalizationExclusionReason(),
        );
        $this->assertSame(
            Topic::PERSONALIZATION_EXCLUSION_UNRESOLVED_OWNERSHIP,
            $unresolvedOwnershipTopic->resolvePersonalizationExclusionReason(),
        );
    }
}