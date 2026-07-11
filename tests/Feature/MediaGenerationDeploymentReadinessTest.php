<?php

namespace Tests\Feature;

use App\MediaGeneration\MediaDeliveryResponseSchema;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use Illuminate\Http\Client\Request;
use Illuminate\Support\Facades\Http;
use Tests\TestCase;

class MediaGenerationDeploymentReadinessTest extends TestCase
{
    public function test_smoke_python_service_command_reports_healthy_service(): void
    {
        config()->set('services.media_generation.python.base_url', 'https://python.example');
        config()->set('services.media_generation.python.health_path', '/v1/health');

        Http::preventStrayRequests();
        Http::fake([
            'https://python.example/v1/health' => Http::response([
                'schema_version' => 'media_generator_health.v1',
                'status' => 'ok',
                'service' => 'klass-media-generator',
                'version' => '0.1.0',
                'supported_formats' => ['docx', 'pdf', 'pptx'],
                'contracts' => [
                    'generation_spec' => 'media_generation_spec.v1',
                    'artifact_metadata' => 'media_generator_output_metadata.v1',
                    'response' => 'media_generator_response.v1',
                ],
                'auth' => [
                    'signature_algorithm' => 'hmac-sha256',
                    'configured' => true,
                    'rotation_enabled' => true,
                    'accepted_secret_count' => 2,
                    'max_request_age_seconds' => 300,
                ],
            ], 200),
        ]);

        $this->artisan('media-generation:smoke-python-service')
            ->expectsOutput('Python media generator service is reachable and healthy.')
            ->expectsOutput('Service: klass-media-generator')
            ->expectsOutput('Version: 0.1.0')
            ->expectsOutput('Health path: /v1/health')
            ->expectsOutput('Supported formats: docx, pdf, pptx')
            ->expectsOutput('Auth configured: yes')
            ->expectsOutput('Rotation enabled: yes')
            ->assertExitCode(0);

        Http::assertSentCount(1);
    }

    public function test_smoke_python_service_command_fails_when_auth_is_not_configured(): void
    {
        config()->set('services.media_generation.python.base_url', 'https://python.example');
        config()->set('services.media_generation.python.health_path', '/v1/health');

        Http::preventStrayRequests();
        Http::fake([
            'https://python.example/v1/health' => Http::response([
                'schema_version' => 'media_generator_health.v1',
                'status' => 'ok',
                'service' => 'klass-media-generator',
                'version' => '0.1.0',
                'supported_formats' => ['docx', 'pdf', 'pptx'],
                'contracts' => [
                    'generation_spec' => 'media_generation_spec.v1',
                    'artifact_metadata' => 'media_generator_output_metadata.v1',
                    'response' => 'media_generator_response.v1',
                ],
                'auth' => [
                    'signature_algorithm' => 'hmac-sha256',
                    'configured' => false,
                    'rotation_enabled' => false,
                    'accepted_secret_count' => 0,
                    'max_request_age_seconds' => 300,
                ],
            ], 200),
        ]);

        $this->artisan('media-generation:smoke-python-service')
            ->expectsOutput('Python media generator health payload reports auth.configured=false.')
            ->expectsOutput('health_path: /v1/health')
            ->assertExitCode(1);

        Http::assertSentCount(1);
    }

    public function test_smoke_llm_adapter_command_reports_healthy_service(): void
    {
        config()->set('services.media_generation.llm_adapter.base_url', 'https://llm.example');
        config()->set('services.media_generation.llm_adapter.health_path', '/v1/health');

        Http::preventStrayRequests();
        Http::fake([
            'https://llm.example/health' => Http::response($this->llmAdapterHealthPayload(), 200),
            'https://llm.example/v1/health' => Http::response($this->llmAdapterHealthPayload(), 200),
        ]);

        $this->artisan('media-generation:smoke-llm-adapter')
            ->expectsOutput('LLM adapter service is reachable and healthy.')
            ->expectsOutput('Service: klass-llm-adapter')
            ->expectsOutput('Version: 0.1.0')
            ->expectsOutput('Health paths: /health, /v1/health')
            ->expectsOutput('Postgres ready: yes')
            ->expectsOutput('Interpretation provider: gemini')
            ->expectsOutput('Delivery provider: gemini')
            ->expectsOutput('Auth configured: yes')
            ->expectsOutput('Rotation enabled: yes')
            ->assertExitCode(0);

        Http::assertSentCount(2);
    }

    public function test_smoke_llm_adapter_command_exercises_signed_interpret_and_respond_routes(): void
    {
        config()->set('services.media_generation.llm_adapter.base_url', 'https://llm.example');
        config()->set('services.media_generation.llm_adapter.health_path', '/v1/health');
        config()->set('services.media_generation.llm_adapter.shared_secret', 'adapter-shared-secret');
        config()->set('services.media_generation.interpreter.path', '/v1/interpret');
        config()->set('services.media_generation.interpreter.provider', 'llm-adapter');
        config()->set('services.media_generation.interpreter.model', 'adapter-managed');
        config()->set('services.media_generation.delivery.path', '/v1/respond');
        config()->set('services.media_generation.delivery.provider', 'llm-adapter');
        config()->set('services.media_generation.delivery.model', 'adapter-managed');

        Http::preventStrayRequests();
        Http::fake([
            'https://llm.example/health' => Http::response($this->llmAdapterHealthPayload(), 200),
            'https://llm.example/v1/health' => Http::response($this->llmAdapterHealthPayload(), 200),
            'https://llm.example/v1/interpret' => Http::response($this->validInterpretationResponsePayload(), 200, [
                'X-Klass-LLM-Provider' => 'gemini',
                'X-Klass-LLM-Model' => 'gemini-2.0-flash',
                'X-Klass-LLM-Primary-Provider' => 'gemini',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ]),
            'https://llm.example/v1/respond' => Http::response($this->validDeliveryResponsePayload(), 200, [
                'X-Klass-LLM-Provider' => 'gemini',
                'X-Klass-LLM-Model' => 'gemini-2.0-flash',
                'X-Klass-LLM-Primary-Provider' => 'gemini',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ]),
        ]);

        $this->artisan('media-generation:smoke-llm-adapter', [
            '--exercise-routes' => true,
            '--expect-provider' => 'gemini',
        ])
            ->expectsOutput('LLM adapter service is reachable and healthy.')
            ->expectsOutput('Interpret smoke provider: gemini')
            ->expectsOutput('Interpret smoke model: gemini-2.0-flash')
            ->expectsOutput('Respond smoke provider: gemini')
            ->expectsOutput('Respond smoke model: gemini-2.0-flash')
            ->assertExitCode(0);

        Http::assertSentCount(4);

        Http::assertSent(function (Request $request): bool {
            if ($request->url() !== 'https://llm.example/v1/interpret') {
                return false;
            }

            $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);

            return ($request->header('X-Klass-Signature-Algorithm')[0] ?? null) === 'hmac-sha256'
                && trim((string) ($request->header('X-Klass-Signature')[0] ?? '')) !== ''
                && data_get($payload, 'request_type') === 'media_prompt_interpretation'
                && data_get($payload, 'model') === 'adapter-managed';
        });

        Http::assertSent(function (Request $request): bool {
            if ($request->url() !== 'https://llm.example/v1/respond') {
                return false;
            }

            $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);

            return ($request->header('X-Klass-Signature-Algorithm')[0] ?? null) === 'hmac-sha256'
                && trim((string) ($request->header('X-Klass-Signature')[0] ?? '')) !== ''
                && data_get($payload, 'request_type') === 'media_delivery_response'
                && data_get($payload, 'model') === 'adapter-managed';
        });
    }

    public function test_smoke_llm_adapter_command_accepts_wrapped_interpretation_response_contract(): void
    {
        config()->set('services.media_generation.llm_adapter.base_url', 'https://llm.example');
        config()->set('services.media_generation.llm_adapter.health_path', '/v1/health');
        config()->set('services.media_generation.llm_adapter.shared_secret', 'adapter-shared-secret');
        config()->set('services.media_generation.interpreter.path', '/v1/interpret');
        config()->set('services.media_generation.interpreter.provider', 'llm-adapter');
        config()->set('services.media_generation.interpreter.model', 'adapter-managed');
        config()->set('services.media_generation.delivery.path', '/v1/respond');
        config()->set('services.media_generation.delivery.provider', 'llm-adapter');
        config()->set('services.media_generation.delivery.model', 'adapter-managed');

        Http::preventStrayRequests();
        Http::fake([
            'https://llm.example/health' => Http::response($this->llmAdapterHealthPayload(), 200),
            'https://llm.example/v1/health' => Http::response($this->llmAdapterHealthPayload(), 200),
            'https://llm.example/v1/interpret' => Http::response([
                'output_text' => json_encode($this->validInterpretationResponsePayload(), JSON_THROW_ON_ERROR),
                'error' => null,
                'response_meta' => [
                    'provider' => 'gemini',
                    'model' => 'gemini-2.0-flash',
                ],
            ], 200, [
                'X-Klass-LLM-Provider' => 'gemini',
                'X-Klass-LLM-Model' => 'gemini-2.0-flash',
                'X-Klass-LLM-Primary-Provider' => 'gemini',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ]),
            'https://llm.example/v1/respond' => Http::response($this->validDeliveryResponsePayload(), 200, [
                'X-Klass-LLM-Provider' => 'gemini',
                'X-Klass-LLM-Model' => 'gemini-2.0-flash',
                'X-Klass-LLM-Primary-Provider' => 'gemini',
                'X-Klass-LLM-Fallback-Used' => 'false',
            ]),
        ]);

        $this->artisan('media-generation:smoke-llm-adapter', [
            '--exercise-routes' => true,
            '--expect-provider' => 'gemini',
        ])
            ->expectsOutput('LLM adapter service is reachable and healthy.')
            ->expectsOutput('Interpret smoke provider: gemini')
            ->expectsOutput('Respond smoke provider: gemini')
            ->assertExitCode(0);
    }

    public function test_smoke_llm_adapter_command_fails_when_postgres_is_not_ready(): void
    {
        config()->set('services.media_generation.llm_adapter.base_url', 'https://llm.example');
        config()->set('services.media_generation.llm_adapter.health_path', '/v1/health');

        Http::preventStrayRequests();
        Http::fake([
            'https://llm.example/health' => Http::response($this->llmAdapterHealthPayload([
                'status' => 'degraded',
                'ready' => false,
                'dependencies' => [
                    'postgres' => [
                        'configured' => true,
                        'ready' => false,
                        'error' => [
                            'code' => 'connection_failed',
                        ],
                    ],
                ],
                'governance' => [
                    'ready' => false,
                ],
            ]), 503),
        ]);

        $this->artisan('media-generation:smoke-llm-adapter')
            ->expectsOutput('LLM adapter health payload reports Postgres is not ready.')
            ->expectsOutput('health_path: /health')
            ->assertExitCode(1);

        Http::assertSentCount(1);
    }

    /**
     * @param  array<string, mixed>  $overrides
     * @return array<string, mixed>
     */
    private function llmAdapterHealthPayload(array $overrides = []): array
    {
        return array_replace_recursive([
            'schema_version' => 'llm_adapter_health.v1',
            'status' => 'ready',
            'ready' => true,
            'service_name' => 'klass-llm-adapter',
            'service_version' => '0.1.0',
            'dependencies' => [
                'postgres' => [
                    'configured' => true,
                    'ready' => true,
                    'driver' => 'postgresql',
                    'host' => 'db.example',
                ],
                'providers' => [
                    'interpretation' => [
                        'route' => 'interpret',
                        'provider' => 'gemini',
                        'ready' => true,
                        'missing_settings' => [],
                    ],
                    'delivery' => [
                        'route' => 'respond',
                        'provider' => 'gemini',
                        'ready' => true,
                        'missing_settings' => [],
                    ],
                ],
            ],
            'auth' => [
                'configured' => true,
                'ready' => true,
                'signature_algorithm' => 'hmac-sha256',
                'rotation_enabled' => true,
                'accepted_secret_count' => 2,
                'max_request_age_seconds' => 300,
            ],
            'governance' => [
                'ready' => true,
                'budget_warning_ratio' => 0.8,
                'routes' => [
                    [
                        'route' => 'interpret',
                        'enabled' => true,
                        'request_limit_per_minute' => 30,
                        'request_limit_per_hour' => 600,
                        'budget_status' => 'healthy',
                        'exhausted_action' => 'deny',
                    ],
                    [
                        'route' => 'respond',
                        'enabled' => true,
                        'request_limit_per_minute' => 60,
                        'request_limit_per_hour' => 1200,
                        'budget_status' => 'healthy',
                        'exhausted_action' => 'degrade',
                    ],
                ],
            ],
        ], $overrides);
    }

    /**
     * @return array<string, mixed>
     */
    private function validInterpretationResponsePayload(): array
    {
        return [
            'schema_version' => MediaPromptInterpretationSchema::VERSION,
            'teacher_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
            'language' => 'id',
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => 'Create a printable classroom handout.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => [
                'Siswa memahami pecahan sederhana.',
            ],
            'constraints' => [
                'preferred_output_type' => 'pdf',
                'max_duration_minutes' => 40,
                'must_include' => ['contoh soal'],
                'avoid' => ['istilah kompleks'],
                'tone' => 'supportive',
            ],
            'output_type_candidates' => [
                ['type' => 'pdf', 'score' => 0.82, 'reason' => 'Format printable paling cocok.'],
                ['type' => 'docx', 'score' => 0.61, 'reason' => 'Dokumen editable tetap relevan.'],
                ['type' => 'pptx', 'score' => 0.29, 'reason' => 'Slide hanya alternatif sekunder.'],
            ],
            'resolved_output_type_reasoning' => 'PDF paling cocok untuk distribusi classroom handout yang stabil.',
            'document_blueprint' => [
                'title' => 'Handout Pecahan Kelas 5',
                'summary' => 'Ringkasan pecahan dasar untuk pengantar dan latihan singkat.',
                'sections' => [
                    [
                        'title' => 'Konsep Dasar',
                        'purpose' => 'Memperkenalkan pecahan sederhana.',
                        'bullets' => ['Pembilang', 'Penyebut'],
                        'estimated_length' => 'short',
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
                'tone' => 'supportive',
                'format_preferences' => ['printable', 'structured'],
                'visual_density' => 'medium',
            ],
            'assets' => [],
            'assessment_or_activity_blocks' => [],
            'teacher_delivery_summary' => 'Gunakan handout ini untuk pengantar lalu lanjutkan ke latihan singkat.',
            'confidence' => [
                'score' => 0.91,
                'label' => 'high',
                'rationale' => 'Prompt jelas dan menyebut kebutuhan printable handout.',
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
    private function validDeliveryResponsePayload(): array
    {
        return [
            'schema_version' => MediaDeliveryResponseSchema::VERSION,
            'title' => 'Handout Pecahan Kelas 5 siap digunakan',
            'preview_summary' => 'Media siap digunakan untuk penguatan konsep dan latihan singkat.',
            'teacher_message' => 'Tinjau satu contoh soal terlebih dahulu sebelum file dibagikan ke siswa.',
            'recommended_next_steps' => [
                'Buka file hasil akhir sebelum dibagikan.',
            ],
            'classroom_tips' => [
                'Mulai dari contoh konkret sebelum latihan mandiri.',
            ],
            'artifact' => [
                'output_type' => 'pdf',
                'title' => 'Handout Pecahan Kelas 5',
                'file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                'thumbnail_url' => 'https://example.com/gallery/handout-pecahan-kelas-5.png',
                'mime_type' => 'application/pdf',
                'filename' => 'handout-pecahan-kelas-5.pdf',
            ],
            'publication' => [
                'topic' => [
                    'id' => 'topic-smoke-1',
                    'title' => 'Handout Pecahan Kelas 5',
                ],
                'content' => [
                    'id' => 'content-smoke-1',
                    'title' => 'Handout Pecahan Kelas 5',
                    'type' => 'brief',
                    'media_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                ],
                'recommended_project' => [
                    'id' => 'project-smoke-1',
                    'title' => 'Handout Pecahan Kelas 5',
                    'project_file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                ],
            ],
            'response_meta' => [
                'generated_at' => now()->toISOString(),
                'llm_used' => true,
                'provider' => 'gemini',
                'model' => 'gemini-2.0-flash',
            ],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }
}