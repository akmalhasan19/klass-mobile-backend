<?php

namespace Tests\Feature;

use App\MediaGeneration\MediaArtifactMetadataContract;
use App\MediaGeneration\MediaContentDraftSchema;
use App\MediaGeneration\MediaDeliveryResponseSchema;
use App\MediaGeneration\MediaGenerationErrorCode;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\MediaGeneration\MediaGenerationServiceException;
use App\MediaGeneration\MediaGenerationSpecContract;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\Models\MediaGeneration;
use App\Models\SubSubject;
use App\Models\User;
use App\Services\MediaContentDraftingService;
use App\Services\MediaDeliveryResponseService;
use App\Services\MediaGenerationAuditTrailService;
use App\Services\MediaGenerationDecisionService;
use App\Services\MediaGenerationWorkflowService;
use App\Services\MediaPublicationService;
use App\Services\MediaPromptInterpretationService;
use App\Services\PythonMediaGeneratorClient;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\Client\Request;
use Illuminate\Support\Facades\Http;
use Mockery;
use Tests\TestCase;

class MediaGenerationOrchestrationServiceTest extends TestCase
{
    use RefreshDatabase;

    public function test_prompt_interpretation_service_calls_adapter_boundary_and_persists_normalized_and_audit_payloads(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
            'preferred_output_type' => 'auto',
            'status' => MediaGenerationLifecycle::INTERPRETING,
        ]);

        config([
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.interpreter.model' => 'adapter-managed',
            'services.media_generation.interpreter.provider' => 'llm-adapter',
        ]);

        Http::fake([
            'https://llm.example/*' => Http::response([
                'choices' => [
                    [
                        'message' => [
                            'content' => json_encode($this->validInterpretationPayload(), JSON_THROW_ON_ERROR),
                        ],
                    ],
                ],
            ], 200, [
                'X-Klass-LLM-Provider' => 'gemini',
                'X-Klass-LLM-Model' => 'gemini-2.0-flash',
                'X-Klass-LLM-Primary-Provider' => 'gemini',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ]),
        ]);

        $result = (new MediaPromptInterpretationService())->interpret($generation);

        $this->assertSame('gemini', $result->llm_provider);
        $this->assertSame('gemini-2.0-flash', $result->llm_model);
        $this->assertSame(MediaPromptInterpretationSchema::VERSION, data_get($result->interpretation_payload, 'schema_version'));
        $this->assertFalse((bool) data_get($result->interpretation_audit_payload, 'response.used_fallback'));
        $this->assertSame('gemini', data_get($result->interpretation_audit_payload, 'provider.name'));
        $this->assertSame('gemini-2.0-flash', data_get($result->interpretation_audit_payload, 'provider.model'));
        $this->assertTrue((bool) data_get($result->interpretation_audit_payload, 'provider.reported_by_adapter'));
        $this->assertSame(
            'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
            data_get($result->interpretation_audit_payload, 'request.input.teacher_prompt')
        );
        $this->assertSame(
            MediaPromptInterpretationSchema::VERSION,
            data_get($result->interpretation_audit_payload, 'response.normalized_payload.schema_version')
        );
        $this->assertNotEmpty(data_get($result->interpretation_audit_payload, 'request_meta.request_id'));
        $this->assertSame('hmac-sha256', data_get($result->interpretation_audit_payload, 'request_meta.signature_algorithm'));

        Http::assertSent(function (Request $request) use ($generation, $subSubject): bool {
            $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);
            $timestamp = $request->header('X-Klass-Request-Timestamp')[0] ?? null;
            $signature = $request->header('X-Klass-Signature')[0] ?? null;
            $requestId = $request->header('X-Request-Id')[0] ?? null;

            return $request->url() === 'https://llm.example/v1/interpret'
                && ($request->header('Authorization')[0] ?? null) === null
                && ($request->header('X-Klass-Generation-Id')[0] ?? null) === $generation->id
                && ($request->header('X-Klass-Signature-Algorithm')[0] ?? null) === 'hmac-sha256'
                && is_string($requestId)
                && trim($requestId) !== ''
                && is_string($timestamp)
                && $timestamp !== ''
                && $signature === hash_hmac('sha256', $timestamp . '.' . $request->body(), 'adapter-shared-secret')
                && $payload === [
                    'request_type' => 'media_prompt_interpretation',
                    'generation_id' => $generation->id,
                    'model' => 'adapter-managed',
                    'instruction' => MediaPromptInterpretationSchema::llmInstruction(),
                    'input' => [
                        'teacher_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
                        'preferred_output_type' => 'auto',
                        'subject_context' => [
                            'id' => $subSubject->subject_id,
                            'name' => $subSubject->subject->name,
                            'slug' => $subSubject->subject->slug,
                        ],
                        'sub_subject_context' => [
                            'id' => $subSubject->id,
                            'name' => $subSubject->name,
                            'slug' => $subSubject->slug,
                        ],
                    ],
                ];
        });
    }

    public function test_prompt_interpretation_service_keeps_adapter_request_contract_stable_when_reported_provider_changes(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
            'preferred_output_type' => 'auto',
            'status' => MediaGenerationLifecycle::INTERPRETING,
        ]);

        config([
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.interpreter.model' => 'adapter-managed',
            'services.media_generation.interpreter.provider' => 'llm-adapter',
        ]);

        $capturedPayloads = [];
        $callIndex = 0;
        $responseHeaders = [
            [
                'X-Klass-LLM-Provider' => 'gemini',
                'X-Klass-LLM-Model' => 'gemini-2.0-flash',
                'X-Klass-LLM-Primary-Provider' => 'gemini',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ],
            [
                'X-Klass-LLM-Provider' => 'openai',
                'X-Klass-LLM-Model' => 'gpt-5.4',
                'X-Klass-LLM-Primary-Provider' => 'openai',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ],
        ];

        Http::fake([
            'https://llm.example/*' => function (Request $request) use (&$capturedPayloads, &$callIndex, $responseHeaders) {
                $capturedPayloads[] = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);

                return Http::response($this->validInterpretationPayload(), 200, $responseHeaders[$callIndex++]);
            },
        ]);

        $firstResult = (new MediaPromptInterpretationService())->interpret($generation->fresh());
        $secondResult = (new MediaPromptInterpretationService())->interpret($generation->fresh());

        $this->assertCount(2, $capturedPayloads);
        $this->assertSame($capturedPayloads[0], $capturedPayloads[1]);
        $this->assertSame('media_prompt_interpretation', data_get($capturedPayloads[0], 'request_type'));
        $this->assertSame('adapter-managed', data_get($capturedPayloads[0], 'model'));
        $this->assertSame('gemini', $firstResult->llm_provider);
        $this->assertSame('gemini-2.0-flash', $firstResult->llm_model);
        $this->assertSame('openai', $secondResult->llm_provider);
        $this->assertSame('gpt-5.4', $secondResult->llm_model);
        $this->assertTrue((bool) data_get($secondResult->interpretation_audit_payload, 'provider.reported_by_adapter'));

        Http::assertSentCount(2);
    }

    public function test_prompt_interpretation_service_records_taxonomy_inference_when_subject_is_omitted(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan PDF pembelajaran IPAS kelas 4 tentang Gaya di Sekitar Kita dengan contoh fenomena dan eksperimen aman.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::INTERPRETING,
        ]);

        config([
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.interpreter.model' => 'adapter-managed',
            'services.media_generation.interpreter.provider' => 'llm-adapter',
        ]);

        $payload = $this->validInterpretationPayload();
        $payload['teacher_prompt'] = $generation->raw_prompt;
        $payload['subject_context'] = null;
        $payload['sub_subject_context'] = null;

        Http::fake([
            'https://llm.example/*' => Http::response([
                'choices' => [
                    [
                        'message' => [
                            'content' => json_encode($payload, JSON_THROW_ON_ERROR),
                        ],
                    ],
                ],
            ], 200),
        ]);

        $result = (new MediaPromptInterpretationService())->interpret($generation);

        $this->assertSame('ipas-sd', data_get($result->interpretation_audit_payload, 'taxonomy_inference.best_match.subject_slug'));
        $this->assertSame('gaya-sekitar-kita-kelas-4', data_get($result->interpretation_audit_payload, 'taxonomy_inference.best_match.sub_subject_slug'));
        $this->assertSame('Ilmu Pengetahuan Alam dan Sosial (IPAS)', data_get($result->interpretation_payload, 'subject_context.subject_name'));
        $this->assertSame('Gaya di Sekitar Kita', data_get($result->interpretation_payload, 'sub_subject_context.sub_subject_name'));

        Http::assertSent(function (Request $request): bool {
            $requestPayload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);

            return str_contains((string) data_get($requestPayload, 'instruction'), 'Internal taxonomy guidance for alignment only:')
                && data_get($requestPayload, 'input.subject_context') === null
                && data_get($requestPayload, 'input.sub_subject_context') === null;
        });
    }

    public function test_prompt_interpretation_service_falls_back_when_llm_returns_partial_contract(): void
    {
        $teacher = User::factory()->teacher()->create();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5.',
            'preferred_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::INTERPRETING,
        ]);

        config([
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
        ]);

        Http::fake([
            'https://llm.example/*' => Http::response([
                'choices' => [
                    [
                        'message' => [
                            'content' => '{"schema_version":"media_prompt_understanding.v1","teacher_prompt":"partial only"}',
                        ],
                    ],
                ],
            ], 200),
        ]);

        $result = (new MediaPromptInterpretationService())->interpret($generation);

        $this->assertTrue((bool) data_get($result->interpretation_payload, 'fallback.triggered'));
        $this->assertSame('pdf', data_get($result->interpretation_payload, 'constraints.preferred_output_type'));
        $this->assertTrue((bool) data_get($result->interpretation_audit_payload, 'response.used_fallback'));
        $this->assertSame(
            MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
            data_get($result->interpretation_audit_payload, 'response.fallback_error.error_code')
        );
    }

    public function test_content_drafting_service_calls_adapter_boundary_and_decision_service_uses_full_material_blocks(): void
    {
        $teacher = User::factory()->teacher()->create();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
            'preferred_output_type' => 'auto',
            'status' => MediaGenerationLifecycle::CLASSIFIED,
            'interpretation_payload' => $this->validInterpretationPayload(),
        ]);

        config([
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.drafting.model' => 'adapter-managed',
            'services.media_generation.drafting.provider' => 'llm-adapter',
        ]);

        Http::fake([
            'https://llm.example/*' => Http::response([
                'choices' => [
                    [
                        'message' => [
                            'content' => json_encode($this->validContentDraftPayload(), JSON_THROW_ON_ERROR),
                        ],
                    ],
                ],
            ], 200, [
                'X-Klass-LLM-Provider' => 'openai',
                'X-Klass-LLM-Model' => 'gpt-5.4',
                'X-Klass-LLM-Primary-Provider' => 'openai',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ]),
        ]);

        $result = (new MediaGenerationDecisionService(new MediaContentDraftingService()))->resolve($generation);

        $this->assertSame('pdf', $result->resolved_output_type);
        $this->assertSame('adapter', data_get($result->decision_payload, 'content_draft.source'));
        $this->assertSame('openai', data_get($result->decision_payload, 'content_draft.adapter_provider'));
        $this->assertSame('gpt-5.4', data_get($result->decision_payload, 'content_draft.adapter_model'));
        $this->assertSame('interpretation_context', data_get($result->decision_payload, 'content_draft.taxonomy_hint.source'));
        $this->assertSame('Matematika', data_get($result->decision_payload, 'content_draft.taxonomy_hint.subject.name'));
        $this->assertSame('paragraph', data_get($result->generation_spec_payload, 'sections.0.body_blocks.0.type'));
        $this->assertStringContainsString(
            'Pecahan senilai adalah dua pecahan yang nilainya sama',
            (string) data_get($result->generation_spec_payload, 'sections.0.body_blocks.0.content')
        );
        $this->assertSame(
            'Gunakan handout ini untuk membangun pemahaman konsep sebelum siswa mengerjakan latihan mandiri.',
            data_get($result->generation_spec_payload, 'teacher_delivery_summary')
        );

        Http::assertSent(function (Request $request) use ($generation): bool {
            $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);

            return $request->url() === 'https://llm.example/v1/draft'
                && ($request->header('X-Klass-Generation-Id')[0] ?? null) === $generation->id
                && data_get($payload, 'request_type') === 'media_content_draft'
                && data_get($payload, 'input.resolved_output_type') === 'pdf'
                && data_get($payload, 'input.interpretation.schema_version') === MediaPromptInterpretationSchema::VERSION
                && data_get($payload, 'input.taxonomy_hint.subject.name') === 'Matematika';
        });
    }

    public function test_media_generation_workflow_service_remains_primary_orchestrator(): void
    {
        $teacher = User::factory()->teacher()->create();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
            'preferred_output_type' => 'auto',
            'status' => MediaGenerationLifecycle::QUEUED,
        ]);

        $sequence = [];

        $interpretationService = Mockery::mock(MediaPromptInterpretationService::class);
        $decisionService = Mockery::mock(MediaGenerationDecisionService::class);
        $pythonMediaGeneratorClient = Mockery::mock(PythonMediaGeneratorClient::class);
        $publicationService = Mockery::mock(MediaPublicationService::class);
        $deliveryResponseService = Mockery::mock(MediaDeliveryResponseService::class);
        $auditTrailService = Mockery::mock(MediaGenerationAuditTrailService::class);

        $auditTrailService->shouldReceive('initialize')
            ->once()
            ->andReturnUsing(function (MediaGeneration $input) use (&$sequence): MediaGeneration {
                $sequence[] = 'initialize';

                return $input;
            });

        $auditTrailService->shouldReceive('transition')
            ->times(6)
            ->andReturnUsing(function (MediaGeneration $input, string $status) use (&$sequence): MediaGeneration {
                $sequence[] = 'transition:' . $status;
                $input->status = $status;

                return $input;
            });

        $interpretationService->shouldReceive('interpret')
            ->once()
            ->andReturnUsing(function (MediaGeneration $input) use (&$sequence): MediaGeneration {
                $sequence[] = 'interpret';
                $input->interpretation_payload = $this->validInterpretationPayload();

                return $input;
            });

        $decisionService->shouldReceive('resolve')
            ->once()
            ->andReturnUsing(function (MediaGeneration $input) use (&$sequence): MediaGeneration {
                $sequence[] = 'decide';
                $input->resolved_output_type = 'pdf';
                $input->generation_spec_payload = MediaGenerationSpecContract::fromInterpretation($this->validInterpretationPayload(), 'pdf');
                $input->decision_payload = [
                    'decision_source' => 'candidate_ranking',
                    'reason_code' => 'highest_score_selected',
                ];

                return $input;
            });

        $pythonMediaGeneratorClient->shouldReceive('generate')
            ->once()
            ->andReturnUsing(function (MediaGeneration $input) use (&$sequence): MediaGeneration {
                $sequence[] = 'generate';
                $input->generator_service_response = [
                    'response' => [
                        'artifact_metadata' => [
                            'schema_version' => MediaArtifactMetadataContract::VERSION,
                        ],
                    ],
                ];

                return $input;
            });

        $publicationService->shouldReceive('publish')
            ->once()
            ->andReturnUsing(function (MediaGeneration $input, callable $afterArtifactPrepared) use (&$sequence): MediaGeneration {
                $sequence[] = 'publish';
                $input = $afterArtifactPrepared($input, [
                    'storage_path' => 'materials/handout-pecahan-kelas-5.pdf',
                    'thumbnail_url' => null,
                ]);
                $input->topic_id = 101;
                $input->content_id = 102;
                $input->recommended_project_id = 103;
                $input->file_url = 'https://example.com/materials/handout-pecahan-kelas-5.pdf';
                $input->mime_type = 'application/pdf';

                return $input;
            });

        $deliveryResponseService->shouldReceive('compose')
            ->once()
            ->andReturnUsing(function (MediaGeneration $input) use (&$sequence): MediaGeneration {
                $sequence[] = 'compose';
                $input->delivery_payload = MediaDeliveryResponseSchema::fallback([
                    'title' => 'Handout Pecahan Kelas 5',
                    'preview_summary' => 'Handout siap dipakai untuk penguatan konsep dan latihan singkat.',
                    'teacher_message' => 'Bagikan file setelah pengantar singkat.',
                    'recommended_next_steps' => ['Buka file dan tinjau contoh soal.'],
                    'classroom_tips' => ['Mulai dari contoh sederhana sebelum latihan.'],
                    'artifact' => [
                        'output_type' => 'pdf',
                        'title' => 'Handout Pecahan Kelas 5',
                        'file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                        'thumbnail_url' => null,
                        'mime_type' => 'application/pdf',
                        'filename' => 'handout-pecahan-kelas-5.pdf',
                    ],
                    'publication' => [
                        'topic' => null,
                        'content' => null,
                        'recommended_project' => null,
                    ],
                ], 'delivery_service_unconfigured');

                return $input;
            });

        $result = (new MediaGenerationWorkflowService(
            $interpretationService,
            $decisionService,
            $pythonMediaGeneratorClient,
            $publicationService,
            $deliveryResponseService,
            $auditTrailService,
        ))->process($generation->id, 2, ['job' => 'phase-1-freeze']);

        $this->assertSame([
            'initialize',
            'transition:interpreting',
            'interpret',
            'decide',
            'transition:classified',
            'transition:generating',
            'generate',
            'transition:uploading',
            'publish',
            'transition:publishing',
            'compose',
            'transition:completed',
        ], $sequence);
        $this->assertSame(MediaGenerationLifecycle::COMPLETED, $result->status);
        $this->assertSame(MediaDeliveryResponseSchema::VERSION, data_get($result->delivery_payload, 'schema_version'));
    }

    public function test_output_decision_service_prioritizes_teacher_override_and_builds_generation_spec(): void
    {
        $teacher = User::factory()->teacher()->create();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
            'preferred_output_type' => 'pptx',
            'status' => MediaGenerationLifecycle::CLASSIFIED,
            'interpretation_payload' => $this->validInterpretationPayload(),
        ]);

        $result = (new MediaGenerationDecisionService())->resolve($generation);

        $this->assertSame('pptx', $result->resolved_output_type);
        $this->assertSame('teacher_override', data_get($result->decision_payload, 'decision_source'));
        $this->assertSame('pptx', data_get($result->generation_spec_payload, 'export_format'));
        $this->assertSame('slide', data_get($result->generation_spec_payload, 'page_or_slide_structure.unit_type'));
    }

    public function test_output_decision_service_uses_deterministic_keyword_signals_for_auto_resolution(): void
    {
        $teacher = User::factory()->teacher()->create();
        $payload = $this->validInterpretationPayload();
        $payload['output_type_candidates'] = [
            [
                'type' => 'docx',
                'score' => 0.60,
                'reason' => 'Editable worksheet remains possible.',
            ],
            [
                'type' => 'pdf',
                'score' => 0.60,
                'reason' => 'Printable handout also fits well.',
            ],
        ];
        $payload['resolved_output_type_reasoning'] = 'Both document formats are plausible for this handout request.';
        $payload['teacher_prompt'] = 'Buatkan handout printable pecahan untuk kelas 5.';

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => $payload['teacher_prompt'],
            'preferred_output_type' => 'auto',
            'status' => MediaGenerationLifecycle::CLASSIFIED,
            'interpretation_payload' => $payload,
        ]);

        $result = (new MediaGenerationDecisionService())->resolve($generation);

        $this->assertSame('pdf', $result->resolved_output_type);
        $this->assertSame('printable_intent_detected', data_get($result->decision_payload, 'reason_code'));
        $this->assertSame('candidate_ranking', data_get($result->decision_payload, 'decision_source'));
    }

    public function test_python_media_generator_client_signs_requests_and_persists_validated_metadata(): void
    {
        $teacher = User::factory()->teacher()->create();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
            'preferred_output_type' => 'pdf',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::GENERATING,
            'generation_spec_payload' => MediaGenerationSpecContract::fromInterpretation($this->validInterpretationPayload(), 'pdf'),
        ]);

        config([
            'services.media_generation.python.base_url' => 'https://python.example',
            'services.media_generation.python.shared_secret' => 'shared-secret',
            'services.media_generation.python.provider' => 'klass-python',
            'services.media_generation.python.model' => 'renderer-v1',
        ]);

        Http::fake([
            'https://python.example/*' => Http::response([
                'schema_version' => 'media_generator_response.v1',
                'request_id' => 'render-123',
                'status' => 'completed',
                'data' => [
                    'generation_id' => $generation->id,
                    'artifact_delivery' => [
                        'kind' => 'temporary_path',
                        'value' => '/tmp/handout-pecahan-kelas-5.pdf',
                    ],
                    'artifact_metadata' => $this->validArtifactMetadata(),
                    'contracts' => [
                        'artifact_metadata' => MediaArtifactMetadataContract::VERSION,
                    ],
                ],
            ], 200),
        ]);

        $result = (new PythonMediaGeneratorClient())->generate($generation);

        $this->assertSame('application/pdf', $result->mime_type);
        $this->assertSame('klass-media-generator', $result->generator_provider);
        $this->assertSame('0.1.0', $result->generator_model);
        $this->assertSame('render-123', data_get($result->generator_service_response, 'response.raw_payload.request_id'));
        $this->assertSame(
            MediaArtifactMetadataContract::VERSION,
            data_get($result->generator_service_response, 'response.artifact_metadata.schema_version')
        );
        $this->assertSame(
            'temporary_path',
            data_get($result->generator_service_response, 'response.raw_payload.data.artifact_delivery.kind')
        );

        Http::assertSent(function (Request $request) use ($generation): bool {
            $timestamp = $request->header('X-Klass-Request-Timestamp')[0] ?? null;
            $signature = $request->header('X-Klass-Signature')[0] ?? null;
            $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);

            return $request->url() === 'https://python.example/v1/generate'
                && ($request->header('X-Klass-Generation-Id')[0] ?? null) === $generation->id
                && $timestamp !== null
                && $signature === hash_hmac('sha256', $timestamp . '.' . $request->body(), 'shared-secret')
                && data_get($payload, 'generation_id') === $generation->id
                && data_get($payload, 'contracts.artifact_metadata') === MediaArtifactMetadataContract::VERSION;
        });
    }

    public function test_python_media_generator_client_classifies_upstream_503_as_service_unavailable(): void
    {
        $teacher = User::factory()->teacher()->create();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5.',
            'preferred_output_type' => 'pdf',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::GENERATING,
            'generation_spec_payload' => MediaGenerationSpecContract::fromInterpretation($this->validInterpretationPayload(), 'pdf'),
        ]);

        config([
            'services.media_generation.python.base_url' => 'https://python.example',
            'services.media_generation.python.shared_secret' => 'shared-secret',
        ]);

        Http::fake([
            'https://python.example/*' => Http::response(['message' => 'temporarily unavailable'], 503),
        ]);

        try {
            (new PythonMediaGeneratorClient())->generate($generation);
            $this->fail('Expected PythonMediaGeneratorClient to throw MediaGenerationServiceException.');
        } catch (MediaGenerationServiceException $exception) {
            $this->assertSame(MediaGenerationErrorCode::PYTHON_SERVICE_UNAVAILABLE, $exception->errorCode());
        }
    }

    public function test_python_media_generator_client_maps_structured_error_hint_to_artifact_invalid(): void
    {
        $teacher = User::factory()->teacher()->create();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan slide pecahan untuk kelas 5.',
            'preferred_output_type' => 'pptx',
            'resolved_output_type' => 'pptx',
            'status' => MediaGenerationLifecycle::GENERATING,
            'generation_spec_payload' => MediaGenerationSpecContract::fromInterpretation($this->validInterpretationPayload(), 'pptx'),
        ]);

        config([
            'services.media_generation.python.base_url' => 'https://python.example',
            'services.media_generation.python.shared_secret' => 'shared-secret',
        ]);

        Http::fake([
            'https://python.example/*' => Http::response([
                'schema_version' => 'media_generator_response.v1',
                'request_id' => 'render-error-123',
                'status' => 'failed',
                'error' => [
                    'code' => 'request_contract_invalid',
                    'message' => 'Incoming request payload failed validation.',
                    'retryable' => true,
                    'laravel_error_code_hint' => MediaGenerationErrorCode::ARTIFACT_INVALID,
                    'details' => [
                        'errors' => ['generation_spec.export_format' => ['Unsupported format.']],
                    ],
                ],
            ], 422),
        ]);

        try {
            (new PythonMediaGeneratorClient())->generate($generation);
            $this->fail('Expected PythonMediaGeneratorClient to throw MediaGenerationServiceException.');
        } catch (MediaGenerationServiceException $exception) {
            $this->assertSame(MediaGenerationErrorCode::ARTIFACT_INVALID, $exception->errorCode());
            $this->assertSame('request_contract_invalid', data_get($exception->context(), 'python_error_code'));
        }
    }

    private function validInterpretationPayload(): array
    {
        return [
            'schema_version' => MediaPromptInterpretationSchema::VERSION,
            'teacher_prompt' => 'Buatkan handout pecahan untuk siswa kelas 5 dengan contoh dan latihan singkat.',
            'language' => 'id',
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => 'Create a printable classroom handout about fractions.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => [
                'Students identify equivalent fractions.',
                'Students solve simple fraction exercises.',
            ],
            'constraints' => [
                'preferred_output_type' => 'auto',
                'max_duration_minutes' => 45,
                'must_include' => ['worked examples', 'short exercises'],
                'avoid' => ['overly technical jargon'],
                'tone' => 'encouraging',
            ],
            'output_type_candidates' => [
                [
                    'type' => 'docx',
                    'score' => 0.61,
                    'reason' => 'Editable worksheet is possible.',
                ],
                [
                    'type' => 'pdf',
                    'score' => 0.72,
                    'reason' => 'Printable handout format matches the prompt best.',
                ],
            ],
            'resolved_output_type_reasoning' => 'PDF best fits a printable classroom handout that should look stable on every device.',
            'document_blueprint' => [
                'title' => 'Handout Pecahan Kelas 5',
                'summary' => 'Handout singkat untuk memperkenalkan pecahan senilai dan latihan dasar.',
                'sections' => [
                    [
                        'title' => 'Tujuan Belajar',
                        'purpose' => 'Frame the lesson and expected outcomes.',
                        'bullets' => ['Memahami pecahan senilai', 'Menyelesaikan latihan dasar'],
                        'estimated_length' => 'short',
                    ],
                    [
                        'title' => 'Contoh dan Latihan',
                        'purpose' => 'Provide guided practice and independent work.',
                        'bullets' => ['Tampilkan satu contoh visual', 'Berikan tiga soal latihan'],
                        'estimated_length' => 'medium',
                    ],
                ],
            ],
            'subject_context' => [
                'subject_name' => 'Matematika',
                'subject_slug' => 'matematika',
            ],
            'sub_subject_context' => [
                'sub_subject_name' => 'Pecahan',
                'sub_subject_slug' => 'pecahan',
            ],
            'target_audience' => [
                'label' => 'Siswa kelas 5',
                'level' => 'elementary',
                'age_range' => '10-11',
            ],
            'requested_media_characteristics' => [
                'tone' => 'encouraging',
                'format_preferences' => ['printable', 'structured'],
                'visual_density' => 'medium',
            ],
            'assets' => [
                [
                    'type' => 'diagram',
                    'description' => 'Fraction circle illustration',
                    'required' => true,
                ],
            ],
            'assessment_or_activity_blocks' => [
                [
                    'title' => 'Latihan Mandiri',
                    'type' => 'activity',
                    'instructions' => 'Kerjakan tiga soal pecahan senilai secara mandiri.',
                ],
            ],
            'teacher_delivery_summary' => 'Gunakan sebagai handout singkat untuk pengenalan materi dan latihan mandiri.',
            'confidence' => [
                'score' => 0.93,
                'label' => 'high',
                'rationale' => 'The prompt explicitly asks for a printable handout with examples and exercises.',
            ],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }

    private function validContentDraftPayload(): array
    {
        return [
            'schema_version' => MediaContentDraftSchema::VERSION,
            'title' => 'Handout Pecahan Kelas 5',
            'summary' => 'Handout ini menjelaskan pecahan senilai melalui contoh sederhana, langkah membandingkan pecahan, dan latihan mandiri singkat.',
            'learning_objectives' => [
                'Students identify equivalent fractions.',
                'Students solve simple fraction exercises.',
            ],
            'sections' => [
                [
                    'title' => 'Tujuan Belajar',
                    'purpose' => 'Frame the lesson and expected outcomes.',
                    'body_blocks' => [
                        [
                            'type' => 'paragraph',
                            'content' => 'Pecahan senilai adalah dua pecahan yang nilainya sama walaupun ditulis dengan angka berbeda. Pada bagian ini, siswa diajak memahami bahwa 1/2 memiliki nilai yang sama dengan 2/4 melalui contoh konkret dan bahasa sederhana.',
                        ],
                        [
                            'type' => 'bullet',
                            'content' => 'Siswa mengenali contoh pecahan senilai pada gambar dan angka.',
                        ],
                    ],
                    'emphasis' => 'short',
                ],
                [
                    'title' => 'Contoh dan Latihan',
                    'purpose' => 'Provide guided practice and independent work.',
                    'body_blocks' => [
                        [
                            'type' => 'paragraph',
                            'content' => 'Guru dapat memulai dengan menunjukkan satu gambar lingkaran yang dibagi menjadi dua bagian sama besar, lalu gambar lain yang dibagi menjadi empat bagian dengan dua bagian diarsir. Dari situ siswa melihat bahwa kedua gambar mewakili nilai yang sama.',
                        ],
                        [
                            'type' => 'checklist',
                            'content' => 'Bandingkan 1/2 dengan 2/4 dan jelaskan mengapa nilainya sama.',
                        ],
                    ],
                    'emphasis' => 'medium',
                ],
            ],
            'teacher_delivery_summary' => 'Gunakan handout ini untuk membangun pemahaman konsep sebelum siswa mengerjakan latihan mandiri.',
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }

    private function validArtifactMetadata(): array
    {
        return [
            'schema_version' => MediaArtifactMetadataContract::VERSION,
            'export_format' => 'pdf',
            'title' => 'Handout Pecahan Kelas 5',
            'filename' => 'handout-pecahan-kelas-5.pdf',
            'extension' => 'pdf',
            'mime_type' => 'application/pdf',
            'size_bytes' => 24576,
            'checksum_sha256' => str_repeat('a', 64),
            'page_count' => 5,
            'artifact_locator' => [
                'kind' => 'temporary_path',
                'value' => '/tmp/handout-pecahan-kelas-5.pdf',
            ],
            'generator' => [
                'name' => 'klass-media-generator',
                'version' => '0.1.0',
            ],
            'warnings' => [],
        ];
    }
}