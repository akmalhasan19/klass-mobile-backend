<?php

namespace Tests\Feature\FreelancerSuggestion;

use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\FreelancerMatch;
use App\Models\MediaGeneration;
use App\Models\Subject;
use App\Models\SubSubject;
use App\Models\User;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class SuggestFreelancersTest extends TestCase
{
    use RefreshDatabase;

    public function test_successful_suggestion_returns_matches_and_stores_them_in_db(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        
        User::factory()->count(10)->create(['role' => User::ROLE_FREELANCER]);

        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/suggest-freelancers");

        $response
            ->assertStatus(200)
            ->assertJsonPath('success', true)
            ->assertJsonPath('message', 'Saran freelancer berhasil didapatkan.')
            ->assertJsonCount(5, 'data'); // default limit is 5

        $data = $response->json('data');

        foreach ($data as $match) {
            $this->assertArrayHasKey('freelancer', $match);
            $this->assertArrayHasKey('match_score', $match);
            
            // Verify db storage
            $this->assertDatabaseHas('freelancer_matches', [
                'media_generation_id' => $generation->id,
                'freelancer_id' => $match['freelancer']['id'],
            ]);
        }
    }

    public function test_custom_limit_parameter_is_respected(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        
        User::factory()->count(10)->create(['role' => User::ROLE_FREELANCER]);

        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/suggest-freelancers?max_suggestions=3");

        $response
            ->assertStatus(200)
            ->assertJsonCount(3, 'data');
    }

    public function test_invalid_generation_returns_404(): void
    {
        $teacher = User::factory()->teacher()->create();
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/invalid-id-123/suggest-freelancers");

        $response->assertStatus(404);
    }

    public function test_generation_not_completed_returns_422(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::QUEUED,
        ]);
        
        Sanctum::actingAs($teacher);

        $response = $this->postJson("/api/v1/media-generations/{$generation->id}/suggest-freelancers");

        $response
            ->assertStatus(422)
            ->assertJsonPath('success', false)
            ->assertJsonPath('message', 'Media generation belum selesai. Tidak dapat mencari freelancer untuk task yang masih diproses.');
    }
}
