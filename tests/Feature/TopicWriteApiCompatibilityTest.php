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

class TopicWriteApiCompatibilityTest extends TestCase
{
    use RefreshDatabase;

    public function test_teacher_create_topic_accepts_mobile_payload_and_auto_normalizes_ownership(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        Sanctum::actingAs($teacher);

        $response = $this->postJson('/api/v1/topics', [
            'title' => 'Algebra Starter Kit',
            'media_url' => 'https://example.com/algebra.jpg',
            'taxonomy' => [
                'subject' => [
                    'id' => $subSubject->subject_id,
                ],
                'sub_subject' => [
                    'id' => $subSubject->id,
                ],
            ],
            'modules' => [
                ['title' => 'Variables'],
            ],
            'source_type' => 'system_topic',
        ]);

        $response
            ->assertCreated()
            ->assertJsonPath('data.title', 'Algebra Starter Kit')
            ->assertJsonPath('data.teacher_id', (string) $teacher->id)
            ->assertJsonPath('data.owner_user_id', $teacher->id)
            ->assertJsonPath('data.ownership_status', Topic::OWNERSHIP_STATUS_NORMALIZED)
            ->assertJsonPath('data.thumbnail_url', 'https://example.com/algebra.jpg')
            ->assertJsonPath('data.sub_subject_id', $subSubject->id)
            ->assertJsonPath('data.subject_id', $subSubject->subject_id)
            ->assertJsonPath('data.taxonomy.subject.slug', 'mathematics')
            ->assertJsonPath('data.taxonomy.sub_subject.slug', 'algebra');

        $topic = Topic::query()->firstOrFail();

        $this->assertSame((string) $teacher->id, $topic->teacher_id);
        $this->assertSame($teacher->id, $topic->owner_user_id);
        $this->assertSame(Topic::OWNERSHIP_STATUS_NORMALIZED, $topic->ownership_status);
        $this->assertSame($subSubject->id, $topic->sub_subject_id);

        $this->getJson('/api/v1/topics?search=Algebra Starter')
            ->assertOk()
            ->assertJsonPath('data.0.teacher_id', (string) $teacher->id)
            ->assertJsonPath('data.0.thumbnail_url', 'https://example.com/algebra.jpg')
            ->assertJsonPath('data.0.sub_subject_id', $subSubject->id)
            ->assertJsonPath('data.0.taxonomy.sub_subject.slug', 'algebra');

        $this->getJson('/api/v1/topics/' . $topic->id)
            ->assertOk()
            ->assertJsonPath('data.teacher_id', (string) $teacher->id)
            ->assertJsonPath('data.thumbnail_url', 'https://example.com/algebra.jpg')
            ->assertJsonPath('data.sub_subject_id', $subSubject->id)
            ->assertJsonPath('data.taxonomy.subject.slug', 'mathematics');
    }

    public function test_admin_update_topic_accepts_taxonomy_payload_and_recomputes_ownership_from_teacher_identifier(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $initialTeacher = User::factory()->teacher()->create();
        $updatedTeacher = User::factory()->teacher()->create([
            'email' => 'updated-owner@example.com',
        ]);
        $admin = User::factory()->admin()->create();

        $initialSubSubject = SubSubject::query()->where('slug', 'geometry')->firstOrFail();
        $updatedSubSubject = SubSubject::query()->where('slug', 'indonesian-history')->firstOrFail();

        $topic = Topic::create([
            'title' => 'Geometry Basics',
            'teacher_id' => (string) $initialTeacher->id,
            'sub_subject_id' => $initialSubSubject->id,
            'thumbnail_url' => 'https://example.com/geometry.jpg',
        ]);

        Sanctum::actingAs($admin);

        $this->putJson('/api/v1/topics/' . $topic->id, [
            'title' => 'History Basics',
            'teacher_id' => strtoupper($updatedTeacher->email),
            'taxonomy' => [
                'subject' => [
                    'id' => $updatedSubSubject->subject_id,
                ],
                'sub_subject' => [
                    'id' => $updatedSubSubject->id,
                ],
            ],
        ])
            ->assertOk()
            ->assertJsonPath('data.title', 'History Basics')
            ->assertJsonPath('data.teacher_id', strtoupper($updatedTeacher->email))
            ->assertJsonPath('data.owner_user_id', $updatedTeacher->id)
            ->assertJsonPath('data.ownership_status', Topic::OWNERSHIP_STATUS_NORMALIZED)
            ->assertJsonPath('data.sub_subject_id', $updatedSubSubject->id)
            ->assertJsonPath('data.subject_id', $updatedSubSubject->subject_id)
            ->assertJsonPath('data.taxonomy.sub_subject.slug', 'indonesian-history');

        $topic->refresh();

        $this->assertSame('History Basics', $topic->title);
        $this->assertSame(strtoupper($updatedTeacher->email), $topic->teacher_id);
        $this->assertSame($updatedTeacher->id, $topic->owner_user_id);
        $this->assertSame(Topic::OWNERSHIP_STATUS_NORMALIZED, $topic->ownership_status);
        $this->assertSame($updatedSubSubject->id, $topic->sub_subject_id);
    }

    public function test_topic_write_rejects_mismatched_subject_and_sub_subject_taxonomy(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $mismatchedSubject = Subject::query()->where('slug', 'science')->firstOrFail();

        Sanctum::actingAs($teacher);

        $this->postJson('/api/v1/topics', [
            'title' => 'Invalid Taxonomy Topic',
            'taxonomy' => [
                'subject' => [
                    'id' => $mismatchedSubject->id,
                ],
                'sub_subject' => [
                    'id' => $subSubject->id,
                ],
            ],
        ])
            ->assertStatus(422)
            ->assertJsonValidationErrors(['sub_subject_id']);
    }
}