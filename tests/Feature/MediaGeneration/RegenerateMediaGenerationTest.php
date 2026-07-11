<?php

namespace Tests\Feature\MediaGeneration;

use App\Jobs\ProcessMediaGenerationJob;
use App\MediaGeneration\MediaGenerationErrorCode;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\MediaGeneration;
use App\Models\User;
use App\Models\Subject;
use App\Models\SubSubject;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class RegenerateMediaGenerationTest extends TestCase
{
    use RefreshDatabase;

    public function test_teacher_can_regenerate_completed_media_generation(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $parentGeneration = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'pdf',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);

        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$parentGeneration->id}/regenerate", [
            'additional_prompt' => 'Tambahkan soal cerita yang lebih menantang.',
        ]);

        $response
            ->assertStatus(202)
            ->assertJsonPath('success', true)
            ->assertJsonPath('data.status', MediaGenerationLifecycle::QUEUED)
            ->assertJsonPath('data.preferred_output_type', 'pdf')
            ->assertJsonPath('data.subject_id', $subSubject->subject_id);

        $newGenerationId = $response->json('data.id');

        $this->assertNotEquals($parentGeneration->id, $newGenerationId);

        $this->assertDatabaseHas('media_generations', [
            'id' => $newGenerationId,
            'generated_from_id' => $parentGeneration->id,
            'is_regeneration' => true,
        ]);

        $newGeneration = MediaGeneration::find($newGenerationId);
        $this->assertStringContainsString('Tambahkan soal cerita yang lebih menantang.', $newGeneration->raw_prompt);

        $this->assertDatabaseHas('jobs', ['queue' => 'media-generation']);
        $queuedJob = DB::table('jobs')->where('queue', 'media-generation')->latest('id')->first();

        $this->assertNotNull($queuedJob);
        $queuedJobPayload = json_decode((string) $queuedJob->payload, true, 512, JSON_THROW_ON_ERROR);

        $this->assertStringContainsString($newGenerationId, (string) data_get($queuedJobPayload, 'data.command'));
    }

    public function test_regenerate_fails_if_parent_not_completed(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $parentGeneration = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout aljabar.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::QUEUED,
        ]);

        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$parentGeneration->id}/regenerate", [
            'additional_prompt' => 'Tambahkan gambar.',
        ]);

        $response
            ->assertStatus(422)
            ->assertJsonPath('success', false)
            ->assertJsonPath('message', 'Media generation belum selesai dan tidak dapat diregenerasi saat ini.');
    }

    public function test_additional_prompt_validation(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $parentGeneration = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);

        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$parentGeneration->id}/regenerate", [
            'additional_prompt' => '',
        ]);

        $response->assertStatus(422);

        $response = $this->postJson("/api/v1/media-generations/{$parentGeneration->id}/regenerate", [
            'additional_prompt' => str_repeat('a', 5001),
        ]);

        $response->assertStatus(422);
    }

    public function test_invalid_parent_id(): void
    {
        $teacher = User::factory()->teacher()->create();
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/invalid-id-123/regenerate", [
            'additional_prompt' => 'test',
        ]);

        $response->assertStatus(404);
    }
}
