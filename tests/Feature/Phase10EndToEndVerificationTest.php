<?php

namespace Tests\Feature;

use App\Jobs\ProcessMediaGenerationJob;
use App\MediaGeneration\MediaArtifactMetadataContract;
use App\MediaGeneration\MediaContentDraftSchema;
use App\MediaGeneration\MediaDeliveryResponseSchema;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\Models\HomepageSection;
use App\Models\MediaGeneration;
use App\Models\RecommendedProject;
use App\Models\SubSubject;
use App\Models\User;
use App\Services\MediaGenerationAuditTrailService;
use App\Services\MediaGenerationWorkflowService;
use App\Services\ThumbnailGeneratorService;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\Client\Request;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Storage;
use Laravel\Sanctum\Sanctum;
use Tests\Concerns\CreatesMediaGenerationArtifacts;
use Tests\TestCase;

class Phase10EndToEndVerificationTest extends TestCase
{
    use CreatesMediaGenerationArtifacts;
    use RefreshDatabase;

    public function test_phase_ten_manual_verification_flow_covers_teacher_submit_generation_publication_feed_and_delivery(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        HomepageSection::create([
            'key' => 'project_recommendations',
            'label' => 'Project Recommendations',
            'position' => 1,
            'is_enabled' => true,
            'data_source' => 'recommended_projects',
        ]);

        Storage::fake('supabase');
        putenv('SUPABASE_URL=https://supabase.example');

        $this->app->instance(ThumbnailGeneratorService::class, $this->fakeThumbnailGeneratorService());

        config([
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.interpreter.provider' => 'llm-adapter',
            'services.media_generation.interpreter.model' => 'adapter-managed',
            'services.media_generation.python.base_url' => 'https://python.example',
            'services.media_generation.python.shared_secret' => 'shared-secret',
            'services.media_generation.python.provider' => 'klass-python',
            'services.media_generation.python.model' => 'renderer-v1',
            'services.media_generation.delivery.provider' => 'llm-adapter',
            'services.media_generation.delivery.model' => 'adapter-managed',
        ]);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();
        $artifactPath = $this->createTempArtifactFile('pptx');

        Sanctum::actingAs($teacher);

        Http::fake([
            'https://llm.example/v1/interpret' => Http::response([
                'choices' => [[
                    'message' => [
                        'content' => json_encode($this->interpretationPayload(), JSON_THROW_ON_ERROR),
                    ],
                ]],
            ], 200),
            'https://llm.example/v1/draft' => Http::response([
                'choices' => [[
                    'message' => [
                        'content' => json_encode($this->contentDraftPayload(), JSON_THROW_ON_ERROR),
                    ],
                ]],
            ], 200, [
                'X-Klass-LLM-Provider' => 'openai',
                'X-Klass-LLM-Model' => 'gpt-5.4',
                'X-Klass-LLM-Primary-Provider' => 'openai',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ]),
            'https://python.example/v1/generate' => Http::response([
                'request_id' => 'phase10-render-123',
                'artifact_metadata' => $this->artifactMetadata($artifactPath),
            ], 200),
            'https://llm.example/v1/respond' => function (Request $request) {
                $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);

                return Http::response([
                    'choices' => [[
                        'message' => [
                            'content' => json_encode([
                                'schema_version' => MediaDeliveryResponseSchema::VERSION,
                                'title' => 'Deck Termodinamika Kelas 11 siap digunakan',
                                'preview_summary' => 'Deck sudah dipublikasikan ke workspace dan homepage recommendation feed.',
                                'teacher_message' => 'Buka slide pembuka terlebih dahulu, lalu bagikan file ke siswa bila dibutuhkan.',
                                'recommended_next_steps' => [
                                    'Buka deck dari kartu hasil untuk mengecek susunan slide.',
                                    'Bagikan file dari CTA share setelah materi pembuka selesai.',
                                ],
                                'classroom_tips' => [
                                    'Gunakan satu contoh kalor sehari-hari saat membuka slide pertama.',
                                ],
                                'artifact' => [
                                    'output_type' => 'pptx',
                                    'title' => 'Deck Termodinamika Kelas 11',
                                    'file_url' => data_get($payload, 'input.artifact.file_url'),
                                    'thumbnail_url' => data_get($payload, 'input.artifact.thumbnail_url'),
                                    'mime_type' => 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
                                    'filename' => 'deck-termodinamika-kelas-11.pptx',
                                ],
                                'publication' => [
                                    'topic' => data_get($payload, 'input.publication.topic'),
                                    'content' => data_get($payload, 'input.publication.content'),
                                    'recommended_project' => data_get($payload, 'input.publication.recommended_project'),
                                ],
                                'response_meta' => [
                                    'generated_at' => now()->toISOString(),
                                    'llm_used' => true,
                                    'provider' => 'llm-gateway',
                                    'model' => 'gpt-5.4',
                                ],
                                'fallback' => [
                                    'triggered' => false,
                                    'reason_code' => null,
                                    'action' => null,
                                ],
                            ], JSON_THROW_ON_ERROR),
                        ],
                    ]],
                ], 200);
            },
        ]);

        $createResponse = $this->postJson('/api/v1/media-generations', [
            'prompt' => 'Buatkan deck termodinamika untuk kelas 11 dengan latihan singkat di akhir.',
            'preferred_output_type' => 'pptx',
            'sub_subject_id' => $subSubject->id,
        ]);

        $createResponse
            ->assertStatus(202)
            ->assertJsonPath('success', true)
            ->assertJsonPath('data.status', MediaGenerationLifecycle::QUEUED)
            ->assertJsonPath('data.preferred_output_type', 'pptx');

        $generationId = $createResponse->json('data.id');

        $this->assertDatabaseHas('media_generations', [
            'id' => $generationId,
            'teacher_id' => $teacher->id,
            'sub_subject_id' => $subSubject->id,
            'status' => MediaGenerationLifecycle::QUEUED,
        ]);

        $job = new ProcessMediaGenerationJob($generationId);
        $job->handle(
            app(MediaGenerationWorkflowService::class),
            app(MediaGenerationAuditTrailService::class),
        );

        $generation = MediaGeneration::query()
            ->with(['topic', 'content', 'recommendedProject'])
            ->findOrFail($generationId);

        $this->assertSame(MediaGenerationLifecycle::COMPLETED, $generation->status);
        $this->assertSame('pptx', $generation->resolved_output_type);
        $this->assertSame(MediaPromptInterpretationSchema::VERSION, data_get($generation->interpretation_payload, 'schema_version'));
        $this->assertSame('pptx', data_get($generation->generation_spec_payload, 'export_format'));
        $this->assertSame('adapter', data_get($generation->decision_payload, 'content_draft.source'));
        $this->assertSame(MediaArtifactMetadataContract::VERSION, data_get($generation->generator_service_response, 'response.artifact_metadata.schema_version'));
        $this->assertSame('pptx', data_get($generation->generator_service_response, 'response.artifact_metadata.extension'));
        $this->assertNotNull($generation->storage_path);
        $this->assertNotNull($generation->file_url);
        $this->assertNotNull($generation->thumbnail_url);
        $this->assertNotNull($generation->topic_id);
        $this->assertNotNull($generation->content_id);
        $this->assertNotNull($generation->recommended_project_id);
        $this->assertSame(2, count(Storage::disk('supabase')->allFiles()));

        $pollResponse = $this->getJson('/api/v1/media-generations/' . $generationId);

        $pollResponse
            ->assertOk()
            ->assertJsonPath('data.status', MediaGenerationLifecycle::COMPLETED)
            ->assertJsonPath('data.resolved_output_type', 'pptx')
            ->assertJsonPath('data.publication.topic.title', 'Deck Termodinamika Kelas 11')
            ->assertJsonPath('data.publication.recommended_project.source_type', RecommendedProject::SOURCE_AI_GENERATED)
            ->assertJsonPath('data.delivery_payload.artifact.file_url', $generation->file_url);

        $this->getJson('/api/v1/topics?search=Deck%20Termodinamika&per_page=5')
            ->assertOk()
            ->assertJsonPath('data.0.title', 'Deck Termodinamika Kelas 11')
            ->assertJsonPath('data.0.sub_subject_id', $subSubject->id);

        $this->getJson('/api/v1/homepage-recommendations')
            ->assertOk()
            ->assertJsonPath('data.0.title', 'Deck Termodinamika Kelas 11')
            ->assertJsonPath('data.0.source_type', RecommendedProject::SOURCE_AI_GENERATED);

        @unlink($artifactPath);
    }

    private function fakeThumbnailGeneratorService(): ThumbnailGeneratorService
    {
        return new class extends ThumbnailGeneratorService
        {
            public function generateFromFile(string $filePath): ?string
            {
                $thumbnailPath = sys_get_temp_dir() . '/phase10_thumb_' . bin2hex(random_bytes(6)) . '.png';
                file_put_contents($thumbnailPath, base64_decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4//8/AwAI/AL+X2VINwAAAABJRU5ErkJggg=='));

                return $thumbnailPath;
            }

            public function generateFromUrl(string $storageUrl): ?string
            {
                return $this->generateFromFile($storageUrl);
            }
        };
    }

    /**
     * @return array<string, mixed>
     */
    private function interpretationPayload(): array
    {
        return [
            'schema_version' => MediaPromptInterpretationSchema::VERSION,
            'teacher_prompt' => 'Buatkan deck termodinamika untuk kelas 11 dengan latihan singkat di akhir.',
            'language' => 'id',
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => 'Create a concise classroom slide deck about thermodynamics.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => [
                'Siswa memahami konsep kalor dan perpindahan energi.',
                'Siswa mencoba latihan singkat setelah paparan konsep.',
            ],
            'constraints' => [
                'preferred_output_type' => 'auto',
                'max_duration_minutes' => 45,
                'must_include' => ['latihan singkat', 'slide pembuka'],
                'avoid' => ['teks terlalu padat'],
                'tone' => 'engaging',
            ],
            'output_type_candidates' => [
                ['type' => 'pdf', 'score' => 0.79, 'reason' => 'PDF bisa dipakai sebagai handout stabil untuk siswa.'],
                ['type' => 'pptx', 'score' => 0.63, 'reason' => 'Slide deck juga cocok untuk presentasi kelas.'],
                ['type' => 'docx', 'score' => 0.42, 'reason' => 'Docx hanya alternatif editable.'],
            ],
            'resolved_output_type_reasoning' => 'Tanpa override teacher, PDF akan lebih stabil untuk distribusi. Namun presentasi tetap relevan untuk paparan kelas.',
            'document_blueprint' => [
                'title' => 'Deck Termodinamika Kelas 11',
                'summary' => 'Slide deck pembuka termodinamika dengan latihan singkat di akhir.',
                'sections' => [
                    [
                        'title' => 'Konsep Kalor',
                        'purpose' => 'Menjelaskan dasar kalor dan perpindahan energi.',
                        'bullets' => ['Definisi kalor', 'Contoh perpindahan energi'],
                        'estimated_length' => 'short',
                    ],
                    [
                        'title' => 'Latihan Cepat',
                        'purpose' => 'Memberikan latihan singkat setelah paparan konsep.',
                        'bullets' => ['Dua soal refleksi', 'Satu diskusi kelas'],
                        'estimated_length' => 'medium',
                    ],
                ],
            ],
            'subject_context' => [
                'subject_name' => 'Science',
                'subject_slug' => 'science',
            ],
            'sub_subject_context' => [
                'sub_subject_name' => 'Thermodynamics',
                'sub_subject_slug' => 'thermodynamics',
            ],
            'target_audience' => [
                'label' => 'Siswa kelas 11',
                'level' => 'high_school',
                'age_range' => '16-17',
            ],
            'requested_media_characteristics' => [
                'tone' => 'engaging',
                'format_preferences' => ['slides', 'classroom_presentation'],
                'visual_density' => 'high',
            ],
            'assets' => [
                [
                    'type' => 'diagram',
                    'description' => 'Ilustrasi perpindahan kalor',
                    'required' => true,
                ],
            ],
            'assessment_or_activity_blocks' => [
                [
                    'title' => 'Latihan Cepat',
                    'type' => 'activity',
                    'instructions' => 'Diskusikan dua contoh perpindahan kalor di kehidupan sehari-hari.',
                ],
            ],
            'teacher_delivery_summary' => 'Gunakan deck ini untuk membuka diskusi kelas sebelum latihan singkat.',
            'confidence' => [
                'score' => 0.87,
                'label' => 'high',
                'rationale' => 'Prompt jelas dan menyebutkan konteks kelas serta kebutuhan latihan.',
            ],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function contentDraftPayload(): array
    {
        return [
            'schema_version' => MediaContentDraftSchema::VERSION,
            'title' => 'Deck Termodinamika Kelas 11',
            'summary' => 'Slide deck pembuka termodinamika dengan contoh kalor sehari-hari dan latihan singkat di akhir.',
            'learning_objectives' => [
                'Siswa memahami konsep kalor dan perpindahan energi.',
                'Siswa mencoba latihan singkat setelah paparan konsep.',
            ],
            'sections' => [
                [
                    'title' => 'Konsep Kalor',
                    'purpose' => 'Menjelaskan dasar kalor dan perpindahan energi.',
                    'body_blocks' => [
                        [
                            'type' => 'paragraph',
                            'content' => 'Kalor adalah energi yang berpindah dari benda bersuhu lebih tinggi ke benda bersuhu lebih rendah. Dalam kehidupan sehari-hari, perpindahan kalor dapat diamati saat sendok logam yang diletakkan di teh panas ikut menjadi hangat.',
                        ],
                        [
                            'type' => 'bullet',
                            'content' => 'Perpindahan kalor terjadi sampai suhu kedua benda mendekati seimbang.',
                        ],
                    ],
                    'emphasis' => 'short',
                ],
                [
                    'title' => 'Latihan Cepat',
                    'purpose' => 'Memberikan latihan singkat setelah paparan konsep.',
                    'body_blocks' => [
                        [
                            'type' => 'paragraph',
                            'content' => 'Setelah mempelajari definisi kalor, siswa diajak menghubungkan konsep tersebut dengan pengalaman sehari-hari. Guru dapat meminta siswa menyebutkan contoh perpindahan kalor yang pernah mereka lihat di rumah atau di sekolah.',
                        ],
                        [
                            'type' => 'checklist',
                            'content' => 'Sebutkan dua contoh perpindahan kalor dalam kehidupan sehari-hari lalu jelaskan arah perpindahan energinya.',
                        ],
                    ],
                    'emphasis' => 'medium',
                ],
            ],
            'teacher_delivery_summary' => 'Gunakan deck ini untuk membuka diskusi kelas sebelum latihan singkat.',
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function artifactMetadata(string $artifactPath): array
    {
        return [
            'schema_version' => MediaArtifactMetadataContract::VERSION,
            'export_format' => 'pptx',
            'title' => 'Deck Termodinamika Kelas 11',
            'filename' => 'deck-termodinamika-kelas-11.pptx',
            'extension' => 'pptx',
            'mime_type' => 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
            'size_bytes' => filesize($artifactPath),
            'checksum_sha256' => hash_file('sha256', $artifactPath),
            'slide_count' => 4,
            'artifact_locator' => [
                'kind' => 'temporary_path',
                'value' => $artifactPath,
            ],
            'generator' => [
                'name' => 'klass-media-generator',
                'version' => '0.1.0',
            ],
            'warnings' => [],
        ];
    }
}