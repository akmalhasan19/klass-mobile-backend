<?php

namespace Tests\Feature\FreelancerHiring;

use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\Content;
use App\Models\MarketplaceTask;
use App\Models\MediaGeneration;
use App\Models\SubSubject;
use App\Models\Subject;
use App\Models\Topic;
use App\Models\User;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class HireFreelancerTest extends TestCase
{
    use RefreshDatabase;

    public function test_auto_suggest_mode_creates_assigned_task(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        $freelancer = User::factory()->create(['role' => User::ROLE_FREELANCER]);
        
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $topic = Topic::create(['title' => 'Test', 'teacher_id' => clone $teacher ? $teacher->id : null, 'sub_subject_id' => clone $subSubject ? $subSubject->id : null]);
        $content = Content::create(['topic_id' => $topic->id, 'type' => 'module', 'title' => 'Test Content', 'data' => []]);

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'content_id' => $content->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/hire-freelancer", [
            'mode' => 'auto_suggest',
            'refinement_description' => 'Tolong rapikan tata letaknya.',
            'selected_freelancer_id' => $freelancer->id,
        ]);

        $response->assertStatus(201);
        $response->assertJsonPath('success', true);
        $response->assertJsonPath('message', 'Task perbaikan berhasil diassign langsung ke freelancer.');

        $this->assertDatabaseHas('marketplace_tasks', [
            'content_id' => $content->id,
            'media_generation_id' => $generation->id,
            'creator_id' => $teacher->id,
            'description' => 'Tolong rapikan tata letaknya.',
            'task_type' => MarketplaceTask::TYPE_SUGGESTION,
            'status' => MarketplaceTask::STATUS_ASSIGNED,
            'suggested_freelancer_id' => $freelancer->id,
        ]);
    }

    public function test_manual_task_mode_creates_open_bid_task(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $topic = Topic::create(['title' => 'Test', 'teacher_id' => clone $teacher ? $teacher->id : null, 'sub_subject_id' => clone $subSubject ? $subSubject->id : null]);
        $content = Content::create(['topic_id' => $topic->id, 'type' => 'module', 'title' => 'Test Content', 'data' => []]);

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'content_id' => $content->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/hire-freelancer", [
            'mode' => 'manual_task',
            'refinement_description' => 'Siapa yang bisa perbaiki gambar?',
        ]);

        $response->assertStatus(201);
        $response->assertJsonPath('success', true);
        $response->assertJsonPath('message', 'Task perbaikan telah diposting ke publik untuk bidding.');

        $this->assertDatabaseHas('marketplace_tasks', [
            'content_id' => $content->id,
            'media_generation_id' => $generation->id,
            'creator_id' => $teacher->id,
            'description' => 'Siapa yang bisa perbaiki gambar?',
            'task_type' => MarketplaceTask::TYPE_BID,
            'status' => MarketplaceTask::STATUS_OPEN_FOR_BID,
            'suggested_freelancer_id' => null,
        ]);
    }

    public function test_missing_refinement_description_fails_validation(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $topic = Topic::create(['title' => 'Test', 'teacher_id' => clone $teacher ? $teacher->id : null, 'sub_subject_id' => clone $subSubject ? $subSubject->id : null]);
        $content = Content::create(['topic_id' => $topic->id, 'type' => 'module', 'title' => 'Test Content', 'data' => []]);

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'content_id' => $content->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/hire-freelancer", [
            'mode' => 'manual_task',
        ]);

        $response->assertStatus(422);
    }

    public function test_invalid_mode_fails_validation(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $topic = Topic::create(['title' => 'Test', 'teacher_id' => clone $teacher ? $teacher->id : null, 'sub_subject_id' => clone $subSubject ? $subSubject->id : null]);
        $content = Content::create(['topic_id' => $topic->id, 'type' => 'module', 'title' => 'Test Content', 'data' => []]);

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'content_id' => $content->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/hire-freelancer", [
            'mode' => 'unknown_mode',
            'refinement_description' => 'test',
        ]);

        $response->assertStatus(422);
    }

    public function test_missing_freelancer_id_for_auto_suggest_fails(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $topic = Topic::create(['title' => 'Test', 'teacher_id' => clone $teacher ? $teacher->id : null, 'sub_subject_id' => clone $subSubject ? $subSubject->id : null]);
        $content = Content::create(['topic_id' => $topic->id, 'type' => 'module', 'title' => 'Test Content', 'data' => []]);

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'content_id' => $content->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/hire-freelancer", [
            'mode' => 'auto_suggest',
            'refinement_description' => 'Test missing ID',
        ]);

        $response->assertStatus(422);
    }

    public function test_invalid_freelancer_id_for_auto_suggest_fails(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $topic = Topic::create(['title' => 'Test', 'teacher_id' => clone $teacher ? $teacher->id : null, 'sub_subject_id' => clone $subSubject ? $subSubject->id : null]);
        $content = Content::create(['topic_id' => $topic->id, 'type' => 'module', 'title' => 'Test Content', 'data' => []]);

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'content_id' => $content->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/hire-freelancer", [
            'mode' => 'auto_suggest',
            'refinement_description' => 'Test invalid ID',
            'selected_freelancer_id' => 'invalid-id-123',
        ]);

        $response->assertStatus(422);
    }
}
