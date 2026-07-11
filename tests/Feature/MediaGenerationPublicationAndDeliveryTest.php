<?php

namespace Tests\Feature;

use App\MediaGeneration\MediaArtifactMetadataContract;
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
use App\Services\FileUploadService;
use App\Services\MediaDeliveryResponseService;
use App\Services\MediaPublicationService;
use App\Services\ThumbnailGeneratorService;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\Client\Request;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Str;
use Tests\Concerns\CreatesMediaGenerationArtifacts;
use Tests\TestCase;

class MediaGenerationPublicationAndDeliveryTest extends TestCase
{
    use CreatesMediaGenerationArtifacts;
    use RefreshDatabase;

    public function test_media_publication_service_uploads_artifact_generates_thumbnail_and_publishes_entities(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        Storage::fake('supabase');
        putenv('SUPABASE_URL=https://supabase.example');

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $artifactPath = $this->createTempArtifactFile('docx');
        $thumbnailService = $this->fakeThumbnailGeneratorService();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'docx',
            'resolved_output_type' => 'docx',
            'status' => MediaGenerationLifecycle::PUBLISHING,
            'interpretation_payload' => $this->interpretationPayload(),
            'generation_spec_payload' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
                'export_format' => 'docx',
                'sections' => [
                    ['title' => 'Konsep Dasar'],
                    ['title' => 'Contoh Soal'],
                ],
            ],
            'generator_service_response' => [
                'response' => [
                    'artifact_metadata' => [
                        'schema_version' => MediaArtifactMetadataContract::VERSION,
                        'export_format' => 'docx',
                        'title' => 'Handout Aljabar Kelas 8',
                        'filename' => 'handout-aljabar-kelas-8.docx',
                        'extension' => 'docx',
                        'mime_type' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
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
                    ],
                ],
            ],
        ]);

        $publishedGeneration = (new MediaPublicationService(app(FileUploadService::class), $thumbnailService))->publish($generation);

        $this->assertNotNull($publishedGeneration->topic_id);
        $this->assertNotNull($publishedGeneration->content_id);
        $this->assertNotNull($publishedGeneration->recommended_project_id);
        $this->assertNotNull($publishedGeneration->storage_path);
        $this->assertNotNull($publishedGeneration->file_url);
        $this->assertNotNull($publishedGeneration->thumbnail_url);
        $this->assertStringStartsWith('materials/', $publishedGeneration->storage_path);
        $this->assertStringContainsString($publishedGeneration->storage_path, $publishedGeneration->file_url);

        $storedFiles = Storage::disk('supabase')->allFiles();

        $this->assertCount(2, $storedFiles);
        $this->assertTrue(collect($storedFiles)->contains(fn (string $path): bool => str_starts_with($path, 'materials/')));
        $this->assertTrue(collect($storedFiles)->contains(fn (string $path): bool => str_starts_with($path, 'gallery/')));
        $this->assertSame('Handout Aljabar Kelas 8', Topic::query()->findOrFail($publishedGeneration->topic_id)->title);
        $this->assertSame(
            $publishedGeneration->file_url,
            Content::query()->findOrFail($publishedGeneration->content_id)->media_url
        );
        $this->assertSame(
            $publishedGeneration->file_url,
            RecommendedProject::query()->findOrFail($publishedGeneration->recommended_project_id)->project_file_url
        );
        $this->assertSame(
            'Handout Aljabar Kelas 8',
            data_get($publishedGeneration->delivery_payload, 'summary.title')
        );

        @unlink($artifactPath);
    }

    public function test_media_publication_service_compensates_uploaded_files_when_publication_fails(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        Storage::fake('supabase');

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $artifactPath = $this->createTempArtifactFile('docx');
        $thumbnailService = $this->fakeThumbnailGeneratorService();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'docx',
            'resolved_output_type' => 'docx',
            'status' => MediaGenerationLifecycle::PUBLISHING,
            'interpretation_payload' => $this->interpretationPayload(),
            'generation_spec_payload' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
                'export_format' => 'docx',
                'sections' => [['title' => 'Konsep Dasar']],
            ],
            'generator_service_response' => [
                'response' => [
                    'artifact_metadata' => [
                        'schema_version' => MediaArtifactMetadataContract::VERSION,
                        'export_format' => 'docx',
                        'title' => 'Handout Aljabar Kelas 8',
                        'filename' => 'handout-aljabar-kelas-8.docx',
                        'extension' => 'docx',
                        'mime_type' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
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
                    ],
                ],
            ],
        ]);

        $service = new class(app(FileUploadService::class), $thumbnailService) extends MediaPublicationService
        {
            protected function createRecommendedProject(MediaGeneration $generation, Topic $topic, Content $content): RecommendedProject
            {
                throw new \RuntimeException('Simulated publication failure.');
            }
        };

        try {
            $service->publish($generation);
            $this->fail('Expected publication to fail.');
        } catch (MediaGenerationServiceException $exception) {
            $this->assertSame(MediaGenerationErrorCode::PUBLICATION_FAILED, $exception->errorCode());
        }

        $this->assertSame([], Storage::disk('supabase')->allFiles());
        $this->assertSame(0, Topic::query()->count());
        $this->assertSame(0, Content::query()->count());
        $this->assertSame(0, RecommendedProject::query()->count());

        @unlink($artifactPath);
    }

    public function test_media_publication_service_rejects_corrupt_office_artifact_before_upload(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        Storage::fake('supabase');

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $artifactPath = sys_get_temp_dir() . '/media_generation_corrupt_' . Str::random(12) . '.docx';
        file_put_contents($artifactPath, 'not a valid office package');

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'docx',
            'resolved_output_type' => 'docx',
            'status' => MediaGenerationLifecycle::PUBLISHING,
            'interpretation_payload' => $this->interpretationPayload(),
            'generation_spec_payload' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
                'export_format' => 'docx',
                'sections' => [['title' => 'Konsep Dasar']],
            ],
            'generator_service_response' => [
                'response' => [
                    'artifact_metadata' => [
                        'schema_version' => MediaArtifactMetadataContract::VERSION,
                        'export_format' => 'docx',
                        'title' => 'Handout Aljabar Kelas 8',
                        'filename' => 'handout-aljabar-kelas-8.docx',
                        'extension' => 'docx',
                        'mime_type' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
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
                    ],
                ],
            ],
        ]);

        try {
            (new MediaPublicationService(app(FileUploadService::class), $this->fakeThumbnailGeneratorService()))->publish($generation);
            $this->fail('Expected publication to reject corrupt artifact.');
        } catch (MediaGenerationServiceException $exception) {
            $this->assertSame(MediaGenerationErrorCode::ARTIFACT_INVALID, $exception->errorCode());
        }

        $this->assertSame([], Storage::disk('supabase')->allFiles());

        @unlink($artifactPath);
    }

    public function test_media_publication_service_uses_fallback_thumbnail_visual_when_preview_generation_fails(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        Storage::fake('supabase');
        putenv('SUPABASE_URL=https://supabase.example');

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $artifactPath = $this->createTempArtifactFile('docx');
        $thumbnailService = new class extends ThumbnailGeneratorService
        {
            public function generateFromFile(string $filePath): ?string
            {
                return null;
            }

            public function generateFromUrl(string $storageUrl): ?string
            {
                return null;
            }
        };

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'docx',
            'resolved_output_type' => 'docx',
            'status' => MediaGenerationLifecycle::PUBLISHING,
            'interpretation_payload' => $this->interpretationPayload(),
            'generation_spec_payload' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
                'export_format' => 'docx',
                'sections' => [['title' => 'Konsep Dasar']],
            ],
            'generator_service_response' => [
                'response' => [
                    'artifact_metadata' => [
                        'schema_version' => MediaArtifactMetadataContract::VERSION,
                        'export_format' => 'docx',
                        'title' => 'Handout Aljabar Kelas 8',
                        'filename' => 'handout-aljabar-kelas-8.docx',
                        'extension' => 'docx',
                        'mime_type' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
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
                    ],
                ],
            ],
        ]);

        $publishedGeneration = (new MediaPublicationService(app(FileUploadService::class), $thumbnailService))->publish($generation);
        $galleryFiles = Storage::disk('supabase')->allFiles('gallery');

        $this->assertNotNull($publishedGeneration->thumbnail_url);
        $this->assertCount(1, $galleryFiles);
        $this->assertStringEndsWith('.svg', $galleryFiles[0]);
        $this->assertStringContainsString('<svg', Storage::disk('supabase')->get($galleryFiles[0]));
        $this->assertSame(
            $publishedGeneration->thumbnail_url,
            RecommendedProject::query()->findOrFail($publishedGeneration->recommended_project_id)->thumbnail_url
        );

        @unlink($artifactPath);
    }

    public function test_media_delivery_response_service_calls_adapter_boundary_with_metadata_only_and_saves_final_payload(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $topic = Topic::create([
            'title' => 'Handout Aljabar Kelas 8',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $subSubject->id,
            'thumbnail_url' => 'https://example.com/thumb.png',
            'is_published' => true,
            'order' => 0,
        ]);
        $content = Content::create([
            'topic_id' => $topic->id,
            'type' => 'brief',
            'title' => 'Handout Aljabar Kelas 8',
            'data' => [],
            'media_url' => 'https://example.com/materials/handout-aljabar-kelas-8.docx',
            'is_published' => true,
            'order' => 0,
        ]);
        $project = RecommendedProject::create([
            'title' => 'Handout Aljabar Kelas 8',
            'description' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
            'thumbnail_url' => 'https://example.com/thumb.png',
            'project_file_url' => 'https://example.com/materials/handout-aljabar-kelas-8.docx',
            'ratio' => '16:9',
            'project_type' => 'learning_material',
            'tags' => ['Matematika', 'DOCX'],
            'modules' => ['Konsep Dasar'],
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_reference' => '1',
            'source_payload' => [],
            'display_priority' => 0,
            'is_active' => true,
            'created_by' => $teacher->id,
            'updated_by' => $teacher->id,
        ]);

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'topic_id' => $topic->id,
            'content_id' => $content->id,
            'recommended_project_id' => $project->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'docx',
            'resolved_output_type' => 'docx',
            'status' => MediaGenerationLifecycle::COMPLETED,
            'storage_path' => 'materials/handout-aljabar-kelas-8.docx',
            'file_url' => 'https://example.com/materials/handout-aljabar-kelas-8.docx',
            'thumbnail_url' => 'https://example.com/thumb.png',
            'mime_type' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
            'interpretation_payload' => $this->interpretationPayload(),
            'generation_spec_payload' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
            ],
            'generator_service_response' => [
                'response' => [
                    'artifact_metadata' => [
                        'filename' => 'handout-aljabar-kelas-8.docx',
                    ],
                ],
            ],
        ]);

        config([
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.delivery.provider' => 'llm-adapter',
            'services.media_generation.delivery.model' => 'adapter-managed',
        ]);

        Http::fake([
            'https://llm.example/*' => Http::response([
                'choices' => [
                    [
                        'message' => [
                            'content' => json_encode([
                                'schema_version' => MediaDeliveryResponseSchema::VERSION,
                                'title' => 'Handout Aljabar Kelas 8 siap digunakan',
                                'preview_summary' => 'Handout ini cocok untuk penguatan konsep dan latihan singkat di kelas 8.',
                                'teacher_message' => 'Materi sudah siap digunakan. Tinjau bagian contoh soal sebelum dibagikan ke siswa.',
                                'recommended_next_steps' => [
                                    'Baca cepat struktur materi sebelum kelas dimulai.',
                                    'Bagikan file ke siswa setelah pengantar singkat.',
                                ],
                                'classroom_tips' => [
                                    'Mulai dengan contoh soal sebelum latihan mandiri.',
                                ],
                                'artifact' => [
                                    'output_type' => 'docx',
                                    'title' => 'Handout Aljabar Kelas 8',
                                    'file_url' => 'https://example.com/materials/handout-aljabar-kelas-8.docx',
                                    'thumbnail_url' => 'https://example.com/thumb.png',
                                    'mime_type' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
                                    'filename' => 'handout-aljabar-kelas-8.docx',
                                ],
                                'publication' => [
                                    'topic' => ['id' => (string) $topic->id, 'title' => $topic->title],
                                    'content' => ['id' => (string) $content->id, 'title' => $content->title, 'type' => $content->type, 'media_url' => $content->media_url],
                                    'recommended_project' => ['id' => (string) $project->id, 'title' => $project->title, 'project_file_url' => $project->project_file_url],
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
                    ],
                ],
            ], 200, [
                'X-Klass-LLM-Provider' => 'openai',
                'X-Klass-LLM-Model' => 'gpt-5.4',
                'X-Klass-LLM-Primary-Provider' => 'gemini',
                'X-Klass-LLM-Fallback-Used' => 'true',
                'X-Klass-LLM-Fallback-Reason' => 'provider_rate_limited',
            ]),
        ]);

        $result = (new MediaDeliveryResponseService())->compose($generation);

        $this->assertSame(MediaDeliveryResponseSchema::VERSION, data_get($result->delivery_payload, 'schema_version'));
        $this->assertFalse((bool) data_get($result->delivery_payload, 'fallback.triggered'));
        $this->assertTrue((bool) data_get($result->delivery_payload, 'response_meta.llm_used'));
        $this->assertSame('openai', data_get($result->delivery_payload, 'response_meta.provider'));
        $this->assertSame('gpt-5.4', data_get($result->delivery_payload, 'response_meta.model'));
        $this->assertSame('https://example.com/materials/handout-aljabar-kelas-8.docx', data_get($result->delivery_payload, 'artifact.file_url'));

        Http::assertSent(function (Request $request): bool {
            $payload = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);
            $timestamp = $request->header('X-Klass-Request-Timestamp')[0] ?? null;
            $signature = $request->header('X-Klass-Signature')[0] ?? null;
            $requestId = $request->header('X-Request-Id')[0] ?? null;

            return $request->url() === 'https://llm.example/v1/respond'
                && ($request->header('Authorization')[0] ?? null) === null
                && ($request->header('X-Klass-Generation-Id')[0] ?? null) !== null
                && ($request->header('X-Klass-Signature-Algorithm')[0] ?? null) === 'hmac-sha256'
                && is_string($requestId)
                && trim($requestId) !== ''
                && is_string($timestamp)
                && $timestamp !== ''
                && $signature === hash_hmac('sha256', $timestamp . '.' . $request->body(), 'adapter-shared-secret')
                && data_get($payload, 'input.artifact.file_url') === 'https://example.com/materials/handout-aljabar-kelas-8.docx'
                && data_get($payload, 'input.publication.recommended_project.project_file_url') === 'https://example.com/materials/handout-aljabar-kelas-8.docx'
                && data_get($payload, 'input.artifact.binary') === null
                && data_get($payload, 'input.artifact.base64') === null;
        });
    }

    public function test_media_delivery_response_service_keeps_adapter_request_contract_stable_when_reported_provider_changes(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $topic = Topic::create([
            'title' => 'Handout Aljabar Kelas 8',
            'teacher_id' => (string) $teacher->id,
            'sub_subject_id' => $subSubject->id,
            'thumbnail_url' => 'https://example.com/thumb.png',
            'is_published' => true,
            'order' => 0,
        ]);
        $content = Content::create([
            'topic_id' => $topic->id,
            'type' => 'brief',
            'title' => 'Handout Aljabar Kelas 8',
            'data' => [],
            'media_url' => 'https://example.com/materials/handout-aljabar-kelas-8.docx',
            'is_published' => true,
            'order' => 0,
        ]);
        $project = RecommendedProject::create([
            'title' => 'Handout Aljabar Kelas 8',
            'description' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
            'thumbnail_url' => 'https://example.com/thumb.png',
            'project_file_url' => 'https://example.com/materials/handout-aljabar-kelas-8.docx',
            'ratio' => '16:9',
            'project_type' => 'learning_material',
            'tags' => ['Matematika', 'DOCX'],
            'modules' => ['Konsep Dasar'],
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_reference' => '1',
            'source_payload' => [],
            'display_priority' => 0,
            'is_active' => true,
            'created_by' => $teacher->id,
            'updated_by' => $teacher->id,
        ]);

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'topic_id' => $topic->id,
            'content_id' => $content->id,
            'recommended_project_id' => $project->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'docx',
            'resolved_output_type' => 'docx',
            'status' => MediaGenerationLifecycle::COMPLETED,
            'storage_path' => 'materials/handout-aljabar-kelas-8.docx',
            'file_url' => 'https://example.com/materials/handout-aljabar-kelas-8.docx',
            'thumbnail_url' => 'https://example.com/thumb.png',
            'mime_type' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
            'interpretation_payload' => $this->interpretationPayload(),
            'generation_spec_payload' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
            ],
            'generator_service_response' => [
                'response' => [
                    'artifact_metadata' => [
                        'filename' => 'handout-aljabar-kelas-8.docx',
                    ],
                ],
            ],
        ]);

        config([
            'services.media_generation.llm_adapter.base_url' => 'https://llm.example',
            'services.media_generation.llm_adapter.shared_secret' => 'adapter-shared-secret',
            'services.media_generation.delivery.provider' => 'llm-adapter',
            'services.media_generation.delivery.model' => 'adapter-managed',
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
        $buildResponsePayload = function (string $provider, string $model) use ($topic, $content, $project): array {
            return [
                'schema_version' => MediaDeliveryResponseSchema::VERSION,
                'title' => 'Handout Aljabar Kelas 8 siap digunakan',
                'preview_summary' => 'Handout ini cocok untuk penguatan konsep dan latihan singkat di kelas 8.',
                'teacher_message' => 'Materi sudah siap digunakan. Tinjau contoh soal sebelum dibagikan ke siswa.',
                'recommended_next_steps' => [
                    'Baca cepat struktur materi sebelum kelas dimulai.',
                    'Bagikan file ke siswa setelah pengantar singkat.',
                ],
                'classroom_tips' => [
                    'Mulai dengan contoh soal sebelum latihan mandiri.',
                ],
                'artifact' => [
                    'output_type' => 'docx',
                    'title' => 'Handout Aljabar Kelas 8',
                    'file_url' => 'https://example.com/materials/handout-aljabar-kelas-8.docx',
                    'thumbnail_url' => 'https://example.com/thumb.png',
                    'mime_type' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
                    'filename' => 'handout-aljabar-kelas-8.docx',
                ],
                'publication' => [
                    'topic' => ['id' => (string) $topic->id, 'title' => $topic->title],
                    'content' => ['id' => (string) $content->id, 'title' => $content->title, 'type' => $content->type, 'media_url' => $content->media_url],
                    'recommended_project' => ['id' => (string) $project->id, 'title' => $project->title, 'project_file_url' => $project->project_file_url],
                ],
                'response_meta' => [
                    'generated_at' => now()->toISOString(),
                    'llm_used' => true,
                    'provider' => $provider,
                    'model' => $model,
                ],
                'fallback' => [
                    'triggered' => false,
                    'reason_code' => null,
                    'action' => null,
                ],
            ];
        };

        Http::fake([
            'https://llm.example/*' => function (Request $request) use (&$capturedPayloads, &$callIndex, $responseHeaders, $buildResponsePayload) {
                $capturedPayloads[] = json_decode($request->body(), true, 512, JSON_THROW_ON_ERROR);
                $provider = $callIndex === 0 ? 'gemini' : 'openai';
                $model = $callIndex === 0 ? 'gemini-2.0-flash' : 'gpt-5.4';

                return Http::response($buildResponsePayload($provider, $model), 200, $responseHeaders[$callIndex++]);
            },
        ]);

        $firstResult = (new MediaDeliveryResponseService())->compose($generation->fresh());
        $secondResult = (new MediaDeliveryResponseService())->compose($generation->fresh());

        $this->assertCount(2, $capturedPayloads);
        $this->assertSame($capturedPayloads[0], $capturedPayloads[1]);
        $this->assertSame('media_delivery_response', data_get($capturedPayloads[0], 'request_type'));
        $this->assertSame('adapter-managed', data_get($capturedPayloads[0], 'model'));
        $this->assertNull(data_get($capturedPayloads[0], 'input.artifact.binary'));
        $this->assertNull(data_get($capturedPayloads[0], 'input.artifact.base64'));
        $this->assertSame('gemini', data_get($firstResult->delivery_payload, 'response_meta.provider'));
        $this->assertSame('gemini-2.0-flash', data_get($firstResult->delivery_payload, 'response_meta.model'));
        $this->assertSame('openai', data_get($secondResult->delivery_payload, 'response_meta.provider'));
        $this->assertSame('gpt-5.4', data_get($secondResult->delivery_payload, 'response_meta.model'));

        Http::assertSentCount(2);
    }

    public function test_media_delivery_response_service_uses_fallback_payload_when_delivery_llm_is_not_configured(): void
    {
        $teacher = User::factory()->teacher()->create();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'preferred_output_type' => 'pdf',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
            'file_url' => 'https://example.com/materials/handout-aljabar.pdf',
            'thumbnail_url' => null,
            'mime_type' => 'application/pdf',
            'interpretation_payload' => $this->interpretationPayload(),
            'generation_spec_payload' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
            ],
        ]);

        config([
            'services.media_generation.llm_adapter.base_url' => null,
            'services.media_generation.delivery.base_url' => null,
        ]);

        $result = (new MediaDeliveryResponseService())->compose($generation);

        $this->assertSame(MediaDeliveryResponseSchema::VERSION, data_get($result->delivery_payload, 'schema_version'));
        $this->assertTrue((bool) data_get($result->delivery_payload, 'fallback.triggered'));
        $this->assertFalse((bool) data_get($result->delivery_payload, 'response_meta.llm_used'));
        $this->assertSame('https://example.com/materials/handout-aljabar.pdf', data_get($result->delivery_payload, 'artifact.file_url'));
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
            'teacher_prompt' => 'Buatkan handout aljabar dasar untuk kelas 8.',
            'language' => 'id',
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => 'Create a classroom handout for basic algebra.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => ['Memahami konsep variabel dasar'],
            'constraints' => [
                'preferred_output_type' => 'auto',
                'max_duration_minutes' => 40,
                'must_include' => ['contoh soal'],
                'avoid' => ['istilah terlalu teknis'],
                'tone' => 'supportive',
            ],
            'output_type_candidates' => [
                ['type' => 'docx', 'score' => 0.66, 'reason' => 'Editable handout is suitable for class customization.'],
                ['type' => 'pdf', 'score' => 0.58, 'reason' => 'Printable handout is also acceptable.'],
            ],
            'resolved_output_type_reasoning' => 'DOCX allows the teacher to adapt examples before classroom delivery.',
            'document_blueprint' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Handout singkat aljabar dasar untuk penguatan konsep.',
                'sections' => [
                    [
                        'title' => 'Konsep Dasar',
                        'purpose' => 'Introduce variables and simple expressions.',
                        'bullets' => ['Pengertian variabel', 'Contoh ekspresi sederhana'],
                        'estimated_length' => 'short',
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
                'format_preferences' => ['structured', 'editable'],
                'visual_density' => 'medium',
            ],
            'assets' => [],
            'assessment_or_activity_blocks' => [],
            'teacher_delivery_summary' => 'Gunakan handout ini untuk membuka materi lalu lanjutkan dengan contoh soal bersama.',
            'confidence' => [
                'score' => 0.88,
                'label' => 'high',
                'rationale' => 'Prompt clearly requests a classroom handout.',
            ],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }
}