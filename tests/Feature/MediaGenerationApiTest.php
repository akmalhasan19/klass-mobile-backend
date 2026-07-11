<?php

namespace Tests\Feature;

use App\Jobs\ProcessMediaGenerationJob;
use App\MediaGeneration\MediaGenerationErrorCode;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\MediaGeneration;
use App\Models\SubSubject;
use App\Models\Subject;
use App\Models\User;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class MediaGenerationApiTest extends TestCase
{
    use RefreshDatabase;

    public function test_teacher_can_submit_media_generation_and_poll_its_status(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        Sanctum::actingAs($teacher);

        $response = $this->postJson('/api/v1/media-generations', [
            'prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'pdf',
            'sub_subject_id' => $subSubject->id,
        ]);

        $response
            ->assertStatus(202)
            ->assertJsonPath('success', true)
            ->assertJsonPath('data.status', MediaGenerationLifecycle::QUEUED)
            ->assertJsonPath('data.preferred_output_type', 'pdf')
            ->assertJsonPath('data.resolved_output_type', null)
            ->assertJsonPath('data.subject_id', $subSubject->subject_id)
            ->assertJsonPath('data.sub_subject_id', $subSubject->id)
            ->assertJsonPath('data.status_meta.lifecycle_version', MediaGenerationLifecycle::VERSION)
            ->assertJsonPath('data.status_meta.is_terminal', false)
            ->assertJsonPath('data.error', null);

        $generationId = $response->json('data.id');

        $this->assertDatabaseHas('jobs', ['queue' => 'media-generation']);
        $queuedJob = DB::table('jobs')->where('queue', 'media-generation')->latest('id')->first();

        $this->assertNotNull($queuedJob);
        $queuedJobPayload = json_decode((string) $queuedJob->payload, true, 512, JSON_THROW_ON_ERROR);

        $this->assertSame(ProcessMediaGenerationJob::class, data_get($queuedJobPayload, 'displayName'));
        $this->assertSame(ProcessMediaGenerationJob::class, data_get($queuedJobPayload, 'data.commandName'));
        $this->assertStringContainsString($generationId, (string) data_get($queuedJobPayload, 'data.command'));

        $response->assertJsonPath('data.links.poll', url('/api/v1/media-generations/' . $generationId));

        $this->assertDatabaseHas('media_generations', [
            'id' => $generationId,
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::QUEUED,
        ]);

        $pollResponse = $this->getJson('/api/v1/media-generations/' . $generationId);

        $pollResponse
            ->assertOk()
            ->assertJsonPath('success', true)
            ->assertJsonPath('data.id', $generationId)
            ->assertJsonPath('data.prompt', 'Buatkan handout aljabar dasar untuk kelas 8.')
            ->assertJsonPath('data.status', MediaGenerationLifecycle::QUEUED);

        $duplicateResponse = $this->postJson('/api/v1/media-generations', [
            'prompt' => '  Buatkan handout aljabar dasar untuk kelas 8.  ',
            'preferred_output_type' => 'pdf',
            'sub_subject_id' => $subSubject->id,
        ]);

        $duplicateResponse
            ->assertStatus(202)
            ->assertJsonPath('data.id', $generationId);

        $this->assertDatabaseCount('media_generations', 1);
        $this->assertDatabaseCount('jobs', 1);
    }

    public function test_media_generation_api_requires_teacher_role_and_owned_generation(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $otherTeacher = User::factory()->teacher()->create();
        $admin = User::factory()->admin()->create();
        $subSubject = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan deck termodinamika.',
            'preferred_output_type' => 'pptx',
            'status' => MediaGenerationLifecycle::QUEUED,
        ]);

        $this->postJson('/api/v1/media-generations', [
            'prompt' => 'Buatkan handout tanpa login.',
        ])->assertUnauthorized();

        Sanctum::actingAs($admin);

        $this->postJson('/api/v1/media-generations', [
            'prompt' => 'Admin mencoba submit.',
        ])
            ->assertForbidden()
            ->assertJsonPath('error.code', MediaGenerationErrorCode::TEACHER_ROLE_REQUIRED);

        Sanctum::actingAs($otherTeacher);

        $this->getJson('/api/v1/media-generations/' . $generation->id)
            ->assertNotFound()
            ->assertJsonPath('error.code', MediaGenerationErrorCode::MEDIA_GENERATION_NOT_FOUND);
    }

    public function test_media_generation_create_validation_returns_stable_error_contract(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subject = Subject::query()->where('slug', 'science')->firstOrFail();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        Sanctum::actingAs($teacher);

        $response = $this->postJson('/api/v1/media-generations', [
            'prompt' => '',
            'preferred_output_type' => 'xlsx',
            'subject_id' => $subject->id,
            'sub_subject_id' => $subSubject->id,
        ]);

        $response
            ->assertStatus(422)
            ->assertJsonPath('success', false)
            ->assertJsonPath('message', 'Validasi gagal.')
            ->assertJsonPath('error.code', MediaGenerationErrorCode::VALIDATION_FAILED)
            ->assertJsonPath('error.retryable', false)
            ->assertJsonValidationErrors(['prompt', 'preferred_output_type', 'sub_subject_id']);
    }

    public function test_failed_media_generation_status_exposes_safe_error_payload_without_raw_stack_trace(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'quantum-physics')->firstOrFail();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan modul fisika kuantum.',
            'preferred_output_type' => 'pdf',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::FAILED,
            'error_code' => MediaGenerationErrorCode::PUBLICATION_FAILED,
            'error_message' => 'SQLSTATE[23000]: duplicate key value violates unique constraint',
        ]);

        Sanctum::actingAs($teacher);

        $response = $this->getJson('/api/v1/media-generations/' . $generation->id);

        $response
            ->assertOk()
            ->assertJsonPath('data.status', MediaGenerationLifecycle::FAILED)
            ->assertJsonPath('data.status_meta.is_terminal', true)
            ->assertJsonPath('data.error.code', MediaGenerationErrorCode::PUBLICATION_FAILED)
            ->assertJsonPath('data.error.message', MediaGenerationErrorCode::clientMessage(MediaGenerationErrorCode::PUBLICATION_FAILED))
            ->assertJsonPath('data.error.retryable', true);

        $this->assertStringNotContainsString('SQLSTATE', $response->getContent());
    }

    public function test_media_generation_resource_keeps_polling_shape_stable_for_frontend(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'pdf',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
            'storage_path' => 'materials/handout-aljabar-kelas-8.pdf',
            'file_url' => 'https://example.com/materials/handout-aljabar-kelas-8.pdf',
            'thumbnail_url' => 'https://example.com/gallery/handout-aljabar-kelas-8.svg',
            'mime_type' => 'application/pdf',
            'llm_provider' => 'llm-gateway',
            'llm_model' => 'gpt-5.4',
            'generator_provider' => 'klass-media-generator',
            'generator_model' => '0.1.0',
            'delivery_payload' => [
                'schema_version' => 'media_delivery_response.v1',
                'title' => 'Handout Aljabar Kelas 8 siap digunakan',
                'preview_summary' => 'Handout siap dipakai untuk penguatan konsep dan latihan singkat.',
                'teacher_message' => 'Bagikan file setelah pengantar singkat.',
                'recommended_next_steps' => ['Tinjau file sebelum dibagikan ke siswa.'],
                'classroom_tips' => ['Mulai dari contoh sederhana sebelum latihan.'],
                'artifact' => [
                    'output_type' => 'pdf',
                    'title' => 'Handout Aljabar Kelas 8',
                    'file_url' => 'https://example.com/materials/handout-aljabar-kelas-8.pdf',
                    'thumbnail_url' => 'https://example.com/gallery/handout-aljabar-kelas-8.svg',
                    'mime_type' => 'application/pdf',
                    'filename' => 'handout-aljabar-kelas-8.pdf',
                ],
                'publication' => [
                    'topic' => null,
                    'content' => null,
                    'recommended_project' => null,
                ],
                'response_meta' => [
                    'generated_at' => '2026-04-08T10:00:00Z',
                    'llm_used' => true,
                    'provider' => 'llm-gateway',
                    'model' => 'gpt-5.4',
                ],
                'fallback' => [
                    'triggered' => false,
                    'reason_code' => null,
                    'action' => null,
                ],
            ],
        ]);

        Sanctum::actingAs($teacher);

        $response = $this->getJson('/api/v1/media-generations/' . $generation->id);

        $response
            ->assertOk()
            ->assertJsonPath('data.id', $generation->id)
            ->assertJsonPath('data.status', MediaGenerationLifecycle::COMPLETED)
            ->assertJsonPath('data.status_meta.lifecycle_version', MediaGenerationLifecycle::VERSION)
            ->assertJsonPath('data.status_meta.is_terminal', true)
            ->assertJsonPath('data.artifact.file_url', 'https://example.com/materials/handout-aljabar-kelas-8.pdf')
            ->assertJsonPath('data.delivery_payload.schema_version', 'media_delivery_response.v1')
            ->assertJsonPath('data.links.poll', url('/api/v1/media-generations/' . $generation->id));

        $resource = $response->json('data');

        foreach (['id', 'prompt', 'preferred_output_type', 'resolved_output_type', 'status', 'status_meta', 'artifact', 'publication', 'delivery_payload', 'error', 'links'] as $key) {
            $this->assertArrayHasKey($key, $resource);
        }

        foreach (['lifecycle_version', 'is_terminal', 'retry_behavior'] as $key) {
            $this->assertArrayHasKey($key, $resource['status_meta']);
        }

        foreach (['storage_path', 'file_url', 'thumbnail_url', 'mime_type'] as $key) {
            $this->assertArrayHasKey($key, $resource['artifact']);
        }

        foreach (['topic', 'content', 'recommended_project'] as $key) {
            $this->assertArrayHasKey($key, $resource['publication']);
        }

        $response->assertJsonMissingPath('data.taxonomy_inference');
        $response->assertJsonMissingPath('data.draft_taxonomy_hint');
    }

    public function test_admin_can_review_taxonomy_debug_surface_without_exposing_it_to_teacher_polling(): void
    {
        $teacher = User::factory()->teacher()->create();
        $admin = User::factory()->admin()->create();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout IPAS kelas 4 tentang gaya di sekitar kita.',
            'preferred_output_type' => 'auto',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
            'interpretation_payload' => [
                'subject_context' => [
                    'subject_name' => 'IPAS',
                    'subject_slug' => 'ipas-sd',
                ],
                'sub_subject_context' => [
                    'sub_subject_name' => 'Gaya di Sekitar Kita',
                    'sub_subject_slug' => 'gaya-sekitar-kita-kelas-4',
                ],
            ],
            'interpretation_audit_payload' => [
                'taxonomy_inference' => [
                    'schema_version' => 'media_prompt_taxonomy_inference.v1',
                    'source' => 'subjects.json',
                    'confidence' => [
                        'score' => 0.91,
                        'label' => 'high',
                        'sub_subject_attached' => true,
                    ],
                    'best_match' => [
                        'jenjang' => 'sd',
                        'kelas' => 4,
                        'semester' => 2,
                        'bab' => 6,
                        'subject_name' => 'IPAS',
                        'subject_slug' => 'ipas-sd',
                        'subject_id' => null,
                        'sub_subject_name' => 'Gaya di Sekitar Kita',
                        'sub_subject_slug' => 'gaya-sekitar-kita-kelas-4',
                        'sub_subject_id' => null,
                        'description' => 'Membahas gaya dorong dan tarik dalam kehidupan sehari-hari.',
                        'content_structure' => 'Pengertian, contoh, pengamatan, dan latihan singkat.',
                        'structure_items' => ['Pengertian gaya', 'Contoh gaya dorong dan tarik', 'Latihan sederhana'],
                        'matched_signals' => ['subject_phrase', 'sub_subject_phrase', 'kelas'],
                    ],
                    'candidate_matches' => [],
                ],
            ],
            'decision_payload' => [
                'content_draft' => [
                    'source' => 'adapter',
                    'schema_version' => 'media_content_draft.v1',
                    'draft_fallback_triggered' => false,
                    'draft_fallback_reason_code' => null,
                    'taxonomy_hint' => [
                        'schema_version' => 'media_draft_taxonomy_hint.v1',
                        'source' => 'prompt_inference',
                        'confidence' => [
                            'score' => 0.91,
                            'label' => 'high',
                        ],
                        'subject' => [
                            'id' => null,
                            'name' => 'IPAS',
                            'slug' => 'ipas-sd',
                        ],
                        'sub_subject' => [
                            'id' => null,
                            'subject_id' => null,
                            'name' => 'Gaya di Sekitar Kita',
                            'slug' => 'gaya-sekitar-kita-kelas-4',
                        ],
                        'grade_context' => [
                            'jenjang' => 'sd',
                            'kelas' => '4',
                            'semester' => '2',
                            'bab' => '6',
                        ],
                        'content_guidance' => [
                            'description' => 'Membahas gaya dorong dan tarik dalam kehidupan sehari-hari.',
                            'structure' => 'Pengertian, contoh, pengamatan, dan latihan singkat.',
                            'structure_items' => ['Pengertian gaya', 'Contoh gaya dorong dan tarik', 'Latihan sederhana'],
                        ],
                        'matched_signals' => ['subject_phrase', 'sub_subject_phrase', 'kelas'],
                    ],
                ],
            ],
        ]);

        Sanctum::actingAs($teacher);

        $this->getJson('/api/v1/media-generations/' . $generation->id)
            ->assertOk()
            ->assertJsonMissingPath('data.taxonomy_inference')
            ->assertJsonMissingPath('data.draft_taxonomy_hint');

        $this->getJson('/api/v1/admin/media-generations/' . $generation->id . '/debug-taxonomy')
            ->assertForbidden();

        Sanctum::actingAs($admin);

        $this->getJson('/api/v1/admin/media-generations/' . $generation->id . '/debug-taxonomy')
            ->assertOk()
            ->assertJsonPath('data.id', $generation->id)
            ->assertJsonPath('data.taxonomy_inference.best_match.subject_slug', 'ipas-sd')
            ->assertJsonPath('data.taxonomy_inference.best_match.sub_subject_slug', 'gaya-sekitar-kita-kelas-4')
            ->assertJsonPath('data.draft_taxonomy_hint.source', 'prompt_inference')
            ->assertJsonPath('data.draft_taxonomy_hint.content_guidance.structure_items.0', 'Pengertian gaya')
            ->assertJsonPath('data.drafting.source', 'adapter');
    }

    public function test_media_generation_error_code_registry_locks_stable_phase_three_contract(): void
    {
        $this->assertContains(MediaGenerationErrorCode::VALIDATION_FAILED, MediaGenerationErrorCode::all());
        $this->assertContains(MediaGenerationErrorCode::LLM_CONTRACT_FAILED, MediaGenerationErrorCode::all());
        $this->assertContains(MediaGenerationErrorCode::PYTHON_SERVICE_UNAVAILABLE, MediaGenerationErrorCode::all());
        $this->assertContains(MediaGenerationErrorCode::ARTIFACT_INVALID, MediaGenerationErrorCode::all());
        $this->assertContains(MediaGenerationErrorCode::UPLOAD_FAILED, MediaGenerationErrorCode::all());
        $this->assertContains(MediaGenerationErrorCode::PUBLICATION_FAILED, MediaGenerationErrorCode::all());
        $this->assertSame(422, MediaGenerationErrorCode::httpStatus(MediaGenerationErrorCode::VALIDATION_FAILED));
        $this->assertSame(503, MediaGenerationErrorCode::httpStatus(MediaGenerationErrorCode::PYTHON_SERVICE_UNAVAILABLE));
        $this->assertTrue(MediaGenerationErrorCode::retryable(MediaGenerationErrorCode::UPLOAD_FAILED));
        $this->assertFalse(MediaGenerationErrorCode::retryable(MediaGenerationErrorCode::VALIDATION_FAILED));
    }
}