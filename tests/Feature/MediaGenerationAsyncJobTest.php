<?php

namespace Tests\Feature;

use App\Jobs\ProcessMediaGenerationJob;
use App\MediaGeneration\MediaArtifactMetadataContract;
use App\MediaGeneration\MediaContentDraftSchema;
use App\MediaGeneration\MediaDeliveryResponseSchema;
use App\MediaGeneration\MediaGenerationErrorCode;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\MediaGeneration\MediaGenerationServiceException;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\Models\Content;
use App\Models\MediaGeneration;
use App\Models\RecommendedProject;
use App\Models\SubSubject;
use App\Models\Topic;
use App\Models\User;
use App\Services\MediaGenerationAuditTrailService;
use App\Services\MediaGenerationWorkflowService;
use App\Services\ThumbnailGeneratorService;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\Client\Request;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Storage;
use Tests\Concerns\CreatesMediaGenerationArtifacts;
use Tests\TestCase;

class MediaGenerationAsyncJobTest extends TestCase
{
    use CreatesMediaGenerationArtifacts;
    use RefreshDatabase;

    public function test_process_media_generation_job_runs_full_async_workflow_and_records_audit_trail(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        Storage::fake('supabase');
        putenv('SUPABASE_URL=https://supabase.example');

        $this->app->instance(ThumbnailGeneratorService::class, $this->fakeThumbnailGeneratorService());

        config([
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
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
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $artifactPath = $this->createTempArtifactFile('pdf');

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout printable aljabar dasar untuk kelas 8 dengan contoh soal singkat.',
            'preferred_output_type' => 'auto',
            'status' => MediaGenerationLifecycle::QUEUED,
        ]);

        Http::fake([
            'https://llm.example/v1/interpret' => Http::response([
                'choices' => [[
                    'message' => [
                        'content' => json_encode($this->interpretationPayload(), JSON_THROW_ON_ERROR),
                    ],
                ]],
            ], 200, [
                'X-Klass-LLM-Provider' => 'gemini',
                'X-Klass-LLM-Model' => 'gemini-2.0-flash',
                'X-Klass-LLM-Primary-Provider' => 'gemini',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ]),
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
                'request_id' => 'render-async-123',
                'artifact_metadata' => $this->artifactMetadata($artifactPath),
            ], 200),
            'https://llm.example/v1/respond' => function (Request $request) {
                $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);
                $fileUrl = data_get($payload, 'input.artifact.file_url');
                $thumbnailUrl = data_get($payload, 'input.artifact.thumbnail_url');

                return Http::response([
                    'choices' => [[
                        'message' => [
                            'content' => json_encode([
                                'schema_version' => MediaDeliveryResponseSchema::VERSION,
                                'title' => 'Handout Aljabar Kelas 8 siap digunakan',
                                'preview_summary' => 'Handout printable sudah siap untuk pengantar materi dan latihan cepat.',
                                'teacher_message' => 'Review contoh soal terlebih dahulu sebelum dibagikan ke siswa.',
                                'recommended_next_steps' => [
                                    'Gunakan handout sebagai pembuka sebelum latihan mandiri.',
                                    'Cetak atau bagikan file ke siswa setelah pengantar singkat.',
                                ],
                                'classroom_tips' => [
                                    'Bahas satu contoh soal terlebih dahulu agar siswa memahami pola pengerjaan.',
                                ],
                                'artifact' => [
                                    'output_type' => 'pdf',
                                    'title' => 'Handout Aljabar Kelas 8',
                                    'file_url' => $fileUrl,
                                    'thumbnail_url' => $thumbnailUrl,
                                    'mime_type' => 'application/pdf',
                                    'filename' => 'handout-aljabar-kelas-8.pdf',
                                ],
                                'publication' => [
                                    'topic' => data_get($payload, 'input.publication.topic'),
                                    'content' => data_get($payload, 'input.publication.content'),
                                    'recommended_project' => data_get($payload, 'input.publication.recommended_project'),
                                ],
                                'response_meta' => [
                                    'generated_at' => now()->toISOString(),
                                    'llm_used' => true,
                                    'provider' => 'openai',
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
                ], 200, [
                    'X-Klass-LLM-Provider' => 'openai',
                    'X-Klass-LLM-Model' => 'gpt-5.4',
                    'X-Klass-LLM-Primary-Provider' => 'gemini',
                    'X-Klass-LLM-Fallback-Used' => 'true',
                    'X-Klass-LLM-Fallback-Reason' => 'provider_rate_limited',
                ]);
            },
        ]);

        $job = new ProcessMediaGenerationJob($generation->id);
        $job->handle(
            app(MediaGenerationWorkflowService::class),
            app(MediaGenerationAuditTrailService::class),
        );

        $generation = $generation->fresh(['topic', 'content', 'recommendedProject']);

        $this->assertSame(MediaGenerationLifecycle::COMPLETED, $generation->status);
        $this->assertSame('pdf', $generation->resolved_output_type);
        $this->assertNotNull($generation->topic_id);
        $this->assertNotNull($generation->content_id);
        $this->assertNotNull($generation->recommended_project_id);
        $this->assertNotNull($generation->file_url);
        $this->assertNotNull($generation->thumbnail_url);
        $this->assertSame(MediaDeliveryResponseSchema::VERSION, data_get($generation->delivery_payload, 'schema_version'));
        $this->assertSame('adapter', data_get($generation->decision_payload, 'content_draft.source'));
        $this->assertSame('gemini', data_get($generation->orchestration_audit_payload, 'provider_trace.interpretation.name'));
        $this->assertSame('klass-media-generator', data_get($generation->orchestration_audit_payload, 'provider_trace.generator.name'));
        $this->assertSame('openai', data_get($generation->orchestration_audit_payload, 'provider_trace.delivery.name'));
        $this->assertSame('pdf', data_get($generation->orchestration_audit_payload, 'resolved_output_type'));
        $this->assertIsInt(data_get($generation->orchestration_audit_payload, 'timing.total_duration_ms'));
        $this->assertGreaterThanOrEqual(0, data_get($generation->orchestration_audit_payload, 'timing.total_duration_ms'));
        $this->assertNull(data_get($generation->orchestration_audit_payload, 'latest_error'));
        $this->assertSame(
            [
                MediaGenerationLifecycle::QUEUED,
                MediaGenerationLifecycle::INTERPRETING,
                MediaGenerationLifecycle::CLASSIFIED,
                MediaGenerationLifecycle::GENERATING,
                MediaGenerationLifecycle::UPLOADING,
                MediaGenerationLifecycle::PUBLISHING,
                MediaGenerationLifecycle::COMPLETED,
            ],
            collect(data_get($generation->orchestration_audit_payload, 'status_history', []))
                ->where('event_type', 'status_transition')
                ->pluck('to_status')
                ->filter()
                ->values()
                ->all()
        );
        $this->assertCount(1, Topic::all());
        $this->assertCount(1, Content::all());
        $this->assertCount(1, RecommendedProject::all());
        $this->assertCount(2, Storage::disk('supabase')->allFiles());

        $rerunJob = new ProcessMediaGenerationJob($generation->id);
        $rerunJob->handle(
            app(MediaGenerationWorkflowService::class),
            app(MediaGenerationAuditTrailService::class),
        );

        $this->assertCount(1, Topic::all());
        $this->assertCount(1, Content::all());
        $this->assertCount(1, RecommendedProject::all());

        @unlink($artifactPath);
    }

    public function test_process_media_generation_job_failed_marks_generation_failed_with_safe_audit_error(): void
    {
        $teacher = User::factory()->teacher()->create();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'pdf',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::GENERATING,
        ]);

        $job = new ProcessMediaGenerationJob($generation->id);
        $job->failed(MediaGenerationServiceException::pythonServiceUnavailable(
            'Python service timeout while contacting upstream renderer.',
            ['http_status' => 503, 'response_body' => 'do not expose me']
        ));

        $generation = $generation->fresh();

        $this->assertSame(MediaGenerationLifecycle::FAILED, $generation->status);
        $this->assertSame(MediaGenerationErrorCode::PYTHON_SERVICE_UNAVAILABLE, $generation->error_code);
        $this->assertSame(MediaGenerationErrorCode::PYTHON_SERVICE_UNAVAILABLE, data_get($generation->orchestration_audit_payload, 'latest_error.error_code'));
        $this->assertSame(MediaGenerationServiceException::class, data_get($generation->orchestration_audit_payload, 'latest_error.error_class'));
        $this->assertTrue((bool) data_get($generation->orchestration_audit_payload, 'latest_error.retryable'));
        $this->assertSame(503, data_get($generation->orchestration_audit_payload, 'latest_error.safe_context.http_status'));
        $this->assertNull(data_get($generation->orchestration_audit_payload, 'latest_error.safe_context.response_body'));
        $this->assertSame(
            [
                MediaGenerationLifecycle::GENERATING,
                MediaGenerationLifecycle::FAILED,
            ],
            collect(data_get($generation->orchestration_audit_payload, 'status_history', []))
                ->where('event_type', 'status_transition')
                ->pluck('to_status')
                ->filter()
                ->values()
                ->all()
        );
    }

    public function test_process_media_generation_job_uses_safe_draft_fallback_when_adapter_returns_internal_prompt_text(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        Storage::fake('supabase');
        putenv('SUPABASE_URL=https://supabase.example');

        $this->app->instance(ThumbnailGeneratorService::class, $this->fakeThumbnailGeneratorService());

        config([
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
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
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $artifactPath = $this->createTempArtifactFile('pdf');

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout printable aljabar dasar untuk kelas 8 dengan contoh soal singkat.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::QUEUED,
        ]);

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
                        'content' => json_encode($this->leakyContentDraftPayload(), JSON_THROW_ON_ERROR),
                    ],
                ]],
            ], 200),
            'https://python.example/v1/generate' => Http::response([
                'request_id' => 'render-async-guard-1',
                'artifact_metadata' => $this->artifactMetadata($artifactPath),
            ], 200),
            'https://llm.example/v1/respond' => function (Request $request) {
                $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);

                return Http::response([
                    'choices' => [[
                        'message' => [
                            'content' => json_encode([
                                'schema_version' => MediaDeliveryResponseSchema::VERSION,
                                'title' => 'Handout Aljabar Kelas 8 siap digunakan',
                                'preview_summary' => 'Handout siap dipakai untuk pengantar materi aljabar dasar.',
                                'teacher_message' => 'Tinjau materi akhir sebelum dibagikan ke siswa.',
                                'recommended_next_steps' => [
                                    'Gunakan contoh pada bagian awal sebagai pembuka diskusi kelas.',
                                ],
                                'classroom_tips' => [
                                    'Ajak siswa menjelaskan kembali arti variabel dengan bahasa mereka sendiri.',
                                ],
                                'artifact' => [
                                    'output_type' => 'pdf',
                                    'title' => 'Handout Aljabar Kelas 8',
                                    'file_url' => data_get($payload, 'input.artifact.file_url'),
                                    'thumbnail_url' => data_get($payload, 'input.artifact.thumbnail_url'),
                                    'mime_type' => 'application/pdf',
                                    'filename' => 'handout-aljabar-kelas-8.pdf',
                                ],
                                'publication' => [
                                    'topic' => data_get($payload, 'input.publication.topic'),
                                    'content' => data_get($payload, 'input.publication.content'),
                                    'recommended_project' => data_get($payload, 'input.publication.recommended_project'),
                                ],
                                'response_meta' => [
                                    'generated_at' => now()->toISOString(),
                                    'llm_used' => true,
                                    'provider' => 'openai',
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

        $job = new ProcessMediaGenerationJob($generation->id);
        $job->handle(
            app(MediaGenerationWorkflowService::class),
            app(MediaGenerationAuditTrailService::class),
        );

        $generation = $generation->fresh();
        $sectionText = collect(data_get($generation->generation_spec_payload, 'sections', []))
            ->pluck('body_blocks')
            ->flatten(1)
            ->pluck('content')
            ->implode(' ');

        $this->assertSame(MediaGenerationLifecycle::COMPLETED, $generation->status);
        $this->assertSame('deterministic_fallback', data_get($generation->decision_payload, 'content_draft.source'));
        $this->assertTrue((bool) data_get($generation->decision_payload, 'content_draft.draft_fallback_triggered'));
        $this->assertStringNotContainsString('Return exactly one JSON object', $sectionText);
        $this->assertStringNotContainsString('schema_version', $sectionText);
        $this->assertStringContainsStringIgnoringCase('aljabar', $sectionText);

        @unlink($artifactPath);
    }

    private function fakeThumbnailGeneratorService(): ThumbnailGeneratorService
    {
        return new class extends ThumbnailGeneratorService
        {
            public function generateFromFile(string $filePath): ?string
            {
                $thumbnailPath = sys_get_temp_dir() . '/thumb_' . Str::random(12) . '.png';
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
            'teacher_prompt' => 'Buatkan handout printable aljabar dasar untuk kelas 8 dengan contoh soal singkat.',
            'language' => 'id',
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => 'Create a printable classroom handout for basic algebra.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => [
                'Siswa memahami variabel dan ekspresi sederhana.',
                'Siswa mencoba latihan aljabar dasar.',
            ],
            'constraints' => [
                'preferred_output_type' => 'auto',
                'max_duration_minutes' => 40,
                'must_include' => ['contoh soal', 'latihan singkat'],
                'avoid' => ['istilah terlalu teknis'],
                'tone' => 'supportive',
            ],
            'output_type_candidates' => [
                ['type' => 'pdf', 'score' => 0.81, 'reason' => 'Printable handout is the strongest fit for classroom distribution.'],
                ['type' => 'docx', 'score' => 0.63, 'reason' => 'Editable document remains possible if teacher wants adaptation.'],
            ],
            'resolved_output_type_reasoning' => 'PDF best matches a printable handout that should render consistently on every device.',
            'document_blueprint' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout printable aljabar dasar untuk pengantar materi dan latihan cepat.',
                'sections' => [
                    [
                        'title' => 'Konsep Dasar',
                        'purpose' => 'Memperkenalkan variabel dan ekspresi sederhana.',
                        'bullets' => ['Pengertian variabel', 'Contoh ekspresi sederhana'],
                        'estimated_length' => 'short',
                    ],
                    [
                        'title' => 'Latihan Singkat',
                        'purpose' => 'Memberikan latihan awal setelah penjelasan konsep.',
                        'bullets' => ['Dua contoh soal', 'Tiga soal latihan mandiri'],
                        'estimated_length' => 'medium',
                    ],
                ],
            ],
            'subject_context' => [
                'subject_name' => 'Matematika',
                'subject_slug' => 'mathematics',
            ],
            'sub_subject_context' => [
                'sub_subject_name' => 'Aljabar',
                'sub_subject_slug' => 'algebra',
            ],
            'target_audience' => [
                'label' => 'Siswa kelas 8',
                'level' => 'middle_school',
                'age_range' => '13-14',
            ],
            'requested_media_characteristics' => [
                'tone' => 'supportive',
                'format_preferences' => ['printable', 'structured'],
                'visual_density' => 'medium',
            ],
            'assets' => [],
            'assessment_or_activity_blocks' => [],
            'teacher_delivery_summary' => 'Gunakan handout ini untuk pengantar materi lalu lanjutkan dengan latihan mandiri singkat.',
            'confidence' => [
                'score' => 0.9,
                'label' => 'high',
                'rationale' => 'Prompt explicitly asks for a printable algebra handout.',
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
            'title' => 'Handout Aljabar Kelas 8',
            'summary' => 'Handout printable aljabar dasar untuk pengantar materi dan latihan cepat.',
            'learning_objectives' => [
                'Siswa memahami variabel dan ekspresi sederhana.',
                'Siswa mencoba latihan aljabar dasar.',
            ],
            'sections' => [
                [
                    'title' => 'Konsep Dasar',
                    'purpose' => 'Memperkenalkan variabel dan ekspresi sederhana.',
                    'body_blocks' => [
                        [
                            'type' => 'paragraph',
                            'content' => 'Variabel adalah simbol, biasanya huruf, yang mewakili suatu nilai. Dalam aljabar dasar, siswa mulai belajar bahwa huruf seperti x atau y dapat digunakan untuk menuliskan pola dan hubungan bilangan secara lebih ringkas.',
                        ],
                        [
                            'type' => 'bullet',
                            'content' => 'Contoh ekspresi sederhana: x + 3 dan 2y - 1.',
                        ],
                    ],
                    'emphasis' => 'short',
                ],
                [
                    'title' => 'Latihan Singkat',
                    'purpose' => 'Memberikan latihan awal setelah penjelasan konsep.',
                    'body_blocks' => [
                        [
                            'type' => 'paragraph',
                            'content' => 'Setelah memahami arti variabel, siswa dapat berlatih dengan mengganti huruf menggunakan bilangan tertentu lalu menghitung hasil ekspresi. Guru dapat memandu satu contoh bersama sebelum siswa mencoba secara mandiri.',
                        ],
                        [
                            'type' => 'checklist',
                            'content' => 'Hitung nilai x + 3 jika x = 4, lalu jelaskan langkahnya secara singkat.',
                        ],
                    ],
                    'emphasis' => 'medium',
                ],
            ],
            'teacher_delivery_summary' => 'Gunakan handout ini untuk pengantar materi lalu lanjutkan dengan latihan mandiri singkat.',
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
    private function leakyContentDraftPayload(): array
    {
        $payload = $this->contentDraftPayload();
        $payload['sections'][0]['body_blocks'][0]['content'] = 'Return exactly one JSON object. Use schema_version media_content_draft.v1 and keep body_blocks.content as final output.';
        $payload['sections'][1]['body_blocks'][0]['content'] = 'Do not wrap the JSON in markdown fences. Set fallback.triggered to false.';

        return $payload;
    }

    /**
     * @return array<string, mixed>
     */
    private function artifactMetadata(string $artifactPath): array
    {
        return [
            'schema_version' => MediaArtifactMetadataContract::VERSION,
            'export_format' => 'pdf',
            'title' => 'Handout Aljabar Kelas 8',
            'filename' => 'handout-aljabar-kelas-8.pdf',
            'extension' => 'pdf',
            'mime_type' => 'application/pdf',
            'size_bytes' => filesize($artifactPath),
            'checksum_sha256' => hash_file('sha256', $artifactPath),
            'page_count' => 4,
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