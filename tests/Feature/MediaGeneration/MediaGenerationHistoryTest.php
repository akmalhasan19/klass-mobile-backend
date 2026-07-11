<?php

namespace Tests\Feature\MediaGeneration;

use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\MediaGeneration;
use App\Models\User;
use App\Models\Subject;
use App\Models\SubSubject;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

/**
 * Feature: GET /api/media-generations?parent_id={uuid}
 *
 * Verifies the parent-chain history endpoint introduced for RF-05 (History
 * Generasi). The endpoint must:
 *   - Return parent + all direct children, sorted oldest-first
 *   - Expose generated_from_id & is_regeneration fields in each item
 *   - Reject non-teacher access (403)
 *   - Return 404 when parent_id belongs to another teacher
 *   - Return the 20 most-recent generations when parent_id is omitted
 */
class MediaGenerationHistoryTest extends TestCase
{
    use RefreshDatabase;

    // -------------------------------------------------------------------------
    // 1.1a — Endpoint returns parent + children sorted oldest-first
    // -------------------------------------------------------------------------

    public function test_history_endpoint_returns_parent_and_children_sorted_oldest_first(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher    = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $parent = MediaGeneration::create([
            'teacher_id'          => $teacher->id,
            'subject_id'          => $subSubject->subject_id,
            'sub_subject_id'      => $subSubject->id,
            'raw_prompt'          => 'Buatkan handout aljabar.',
            'preferred_output_type' => 'pdf',
            'status'              => MediaGenerationLifecycle::COMPLETED,
        ]);

        $child1 = MediaGeneration::create([
            'teacher_id'          => $teacher->id,
            'generated_from_id'   => $parent->id,
            'is_regeneration'     => true,
            'subject_id'          => $subSubject->subject_id,
            'sub_subject_id'      => $subSubject->id,
            'raw_prompt'          => 'Buatkan handout aljabar. [+] Lebih visual.',
            'preferred_output_type' => 'pdf',
            'status'              => MediaGenerationLifecycle::COMPLETED,
        ]);

        $child2 = MediaGeneration::create([
            'teacher_id'          => $teacher->id,
            'generated_from_id'   => $parent->id,
            'is_regeneration'     => true,
            'subject_id'          => $subSubject->subject_id,
            'sub_subject_id'      => $subSubject->id,
            'raw_prompt'          => 'Buatkan handout aljabar. [+] Tambahkan soal cerita.',
            'preferred_output_type' => 'pdf',
            'status'              => MediaGenerationLifecycle::QUEUED,
        ]);

        Sanctum::actingAs($teacher);

        $response = $this->getJson("/api/v1/media-generations?parent_id={$parent->id}");

        $response
            ->assertStatus(200)
            ->assertJsonPath('success', true);

        $data = $response->json('data');

        $this->assertCount(3, $data, 'Should return parent + 2 children');

        // Sorted oldest-first: parent, child1, child2
        $this->assertEquals($parent->id,  $data[0]['id']);
        $this->assertEquals($child1->id,  $data[1]['id']);
        $this->assertEquals($child2->id,  $data[2]['id']);
    }

    // -------------------------------------------------------------------------
    // 1.1b — Response includes generated_from_id and is_regeneration fields
    // -------------------------------------------------------------------------

    public function test_history_response_includes_parent_chain_fields(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher    = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $parent = MediaGeneration::create([
            'teacher_id'            => $teacher->id,
            'subject_id'            => $subSubject->subject_id,
            'sub_subject_id'        => $subSubject->id,
            'raw_prompt'            => 'Handout aljabar.',
            'preferred_output_type' => 'pdf',
            'status'                => MediaGenerationLifecycle::COMPLETED,
        ]);

        $child = MediaGeneration::create([
            'teacher_id'            => $teacher->id,
            'generated_from_id'     => $parent->id,
            'is_regeneration'       => true,
            'subject_id'            => $subSubject->subject_id,
            'sub_subject_id'        => $subSubject->id,
            'raw_prompt'            => 'Handout aljabar. [+] Visual.',
            'preferred_output_type' => 'pdf',
            'status'                => MediaGenerationLifecycle::QUEUED,
        ]);

        Sanctum::actingAs($teacher);

        $response = $this->getJson("/api/v1/media-generations?parent_id={$parent->id}");

        $response->assertStatus(200);

        $data = $response->json('data');

        // Parent item: is_regeneration=false, generated_from_id=null
        $this->assertFalse($data[0]['is_regeneration']);
        $this->assertNull($data[0]['generated_from_id']);

        // Child item: is_regeneration=true, generated_from_id=parent->id
        $this->assertTrue($data[1]['is_regeneration']);
        $this->assertEquals($parent->id, $data[1]['generated_from_id']);

        // Verify other required fields present per plan §1.1 verify list
        foreach ($data as $item) {
            $this->assertArrayHasKey('id', $item);
            $this->assertArrayHasKey('prompt', $item);
            $this->assertArrayHasKey('status', $item);
            $this->assertArrayHasKey('created_at', $item);
            $this->assertArrayHasKey('updated_at', $item);
            $this->assertArrayHasKey('is_regeneration', $item);
            $this->assertArrayHasKey('generated_from_id', $item);
        }
    }

    // -------------------------------------------------------------------------
    // 1.1c — Endpoint walks to root when parent_id is a child UUID
    // -------------------------------------------------------------------------

    public function test_history_walks_to_root_when_querying_by_child_id(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher    = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $root = MediaGeneration::create([
            'teacher_id'            => $teacher->id,
            'subject_id'            => $subSubject->subject_id,
            'sub_subject_id'        => $subSubject->id,
            'raw_prompt'            => 'Root generation.',
            'preferred_output_type' => 'pdf',
            'status'                => MediaGenerationLifecycle::COMPLETED,
        ]);

        $child = MediaGeneration::create([
            'teacher_id'            => $teacher->id,
            'generated_from_id'     => $root->id,
            'is_regeneration'       => true,
            'subject_id'            => $subSubject->subject_id,
            'sub_subject_id'        => $subSubject->id,
            'raw_prompt'            => 'Root generation. [+] Revision 1.',
            'preferred_output_type' => 'pdf',
            'status'                => MediaGenerationLifecycle::COMPLETED,
        ]);

        Sanctum::actingAs($teacher);

        // Query by child ID — should still return the full chain (root + child)
        $response = $this->getJson("/api/v1/media-generations?parent_id={$child->id}");

        $response->assertStatus(200);

        $data = $response->json('data');
        $this->assertCount(2, $data);
        $this->assertEquals($root->id,  $data[0]['id']);
        $this->assertEquals($child->id, $data[1]['id']);
    }

    // -------------------------------------------------------------------------
    // 1.1d — parent_id from another teacher returns 404
    // -------------------------------------------------------------------------

    public function test_history_with_other_teacher_parent_id_returns_404(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher      = User::factory()->teacher()->create();
        $otherTeacher = User::factory()->teacher()->create();
        $subSubject   = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $otherGeneration = MediaGeneration::create([
            'teacher_id'            => $otherTeacher->id,
            'subject_id'            => $subSubject->subject_id,
            'sub_subject_id'        => $subSubject->id,
            'raw_prompt'            => 'Other teacher generation.',
            'preferred_output_type' => 'pdf',
            'status'                => MediaGenerationLifecycle::COMPLETED,
        ]);

        Sanctum::actingAs($teacher);

        $response = $this->getJson("/api/v1/media-generations?parent_id={$otherGeneration->id}");

        $response->assertStatus(404);
    }

    // -------------------------------------------------------------------------
    // 1.1e — Freelancer / non-teacher role gets 403
    // -------------------------------------------------------------------------

    public function test_history_endpoint_rejects_non_teacher_role(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $freelancer = User::factory()->freelancer()->create();

        Sanctum::actingAs($freelancer);

        $response = $this->getJson('/api/v1/media-generations?parent_id=any-uuid');

        $response->assertStatus(403);
    }

    // -------------------------------------------------------------------------
    // 1.1f — Without parent_id, returns up to 20 most-recent generations
    // -------------------------------------------------------------------------

    public function test_history_endpoint_returns_recent_generations_when_no_parent_id(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher    = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        // Create 5 generations
        for ($i = 0; $i < 5; $i++) {
            MediaGeneration::create([
                'teacher_id'            => $teacher->id,
                'subject_id'            => $subSubject->subject_id,
                'sub_subject_id'        => $subSubject->id,
                'raw_prompt'            => "Generation #{$i}",
                'preferred_output_type' => 'pdf',
                'status'                => MediaGenerationLifecycle::COMPLETED,
            ]);
        }

        Sanctum::actingAs($teacher);

        $response = $this->getJson('/api/v1/media-generations');

        $response
            ->assertStatus(200)
            ->assertJsonPath('success', true);

        $data = $response->json('data');
        $this->assertCount(5, $data);

        // Each item must expose the parent-chain fields
        foreach ($data as $item) {
            $this->assertArrayHasKey('is_regeneration', $item);
            $this->assertArrayHasKey('generated_from_id', $item);
        }
    }

    // -------------------------------------------------------------------------
    // 1.1g — Isolated generations from other teachers not leaked
    // -------------------------------------------------------------------------

    public function test_history_does_not_leak_generations_from_other_teachers(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher      = User::factory()->teacher()->create();
        $otherTeacher = User::factory()->teacher()->create();
        $subSubject   = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        // Teacher A: 2 generations
        MediaGeneration::create([
            'teacher_id'            => $teacher->id,
            'subject_id'            => $subSubject->subject_id,
            'sub_subject_id'        => $subSubject->id,
            'raw_prompt'            => 'Teacher A gen 1.',
            'preferred_output_type' => 'pdf',
            'status'                => MediaGenerationLifecycle::COMPLETED,
        ]);

        // Teacher B: 3 generations
        for ($i = 0; $i < 3; $i++) {
            MediaGeneration::create([
                'teacher_id'            => $otherTeacher->id,
                'subject_id'            => $subSubject->subject_id,
                'sub_subject_id'        => $subSubject->id,
                'raw_prompt'            => "Teacher B gen {$i}.",
                'preferred_output_type' => 'pdf',
                'status'                => MediaGenerationLifecycle::COMPLETED,
            ]);
        }

        Sanctum::actingAs($teacher);

        $response = $this->getJson('/api/v1/media-generations');

        $response->assertStatus(200);

        // Must only see Teacher A's own generation
        $data = $response->json('data');
        $this->assertCount(1, $data);
        $this->assertEquals($teacher->id, $data[0]['teacher_id']);
    }
}
