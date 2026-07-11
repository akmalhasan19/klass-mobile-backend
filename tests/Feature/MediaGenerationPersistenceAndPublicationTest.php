<?php

namespace Tests\Feature;

use App\Services\FileUploadService;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\Content;
use App\Models\MediaGeneration;
use App\Models\RecommendedProject;
use App\Models\SubSubject;
use App\Models\Subject;
use App\Models\Topic;
use App\Models\User;
use App\Services\MediaGenerationSubmissionService;
use App\Services\MediaPublicationService;
use App\Services\RecommendationAggregationService;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Schema;
use Tests\TestCase;

class MediaGenerationPersistenceAndPublicationTest extends TestCase
{
    use RefreshDatabase;

    public function test_media_publication_related_url_columns_use_text_storage(): void
    {
        $this->assertSame('text', Schema::getColumnType('topics', 'thumbnail_url'));
        $this->assertSame('text', Schema::getColumnType('contents', 'media_url'));
        $this->assertSame('text', Schema::getColumnType('recommended_projects', 'thumbnail_url'));
        $this->assertSame('text', Schema::getColumnType('recommended_projects', 'project_file_url'));
    }

    public function test_media_generation_persistence_supports_phase_two_shape_and_active_duplicate_reuse(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subject = Subject::query()->where('slug', 'mathematics')->firstOrFail();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $service = new MediaGenerationSubmissionService();

        $this->assertTrue(collect([
            'teacher_id',
            'topic_id',
            'content_id',
            'recommended_project_id',
            'raw_prompt',
            'preferred_output_type',
            'resolved_output_type',
            'status',
            'llm_provider',
            'llm_model',
            'generator_provider',
            'generator_model',
            'interpretation_payload',
            'generation_spec_payload',
            'delivery_payload',
            'generator_service_response',
            'storage_path',
            'file_url',
            'thumbnail_url',
            'mime_type',
            'error_code',
            'error_message',
            'request_fingerprint',
            'active_duplicate_key',
        ])->every(fn (string $column): bool => Schema::hasColumn('media_generations', $column)));

        $generation = $service->createOrReuse(
            teacherId: $teacher->id,
            rawPrompt: '  Buatkan handout   aljabar dasar untuk siswa SMP.  ',
            preferredOutputType: 'pdf',
            subjectId: $subject->id,
            subSubjectId: $subSubject->id,
            providerMetadata: [
                'llm_provider' => 'openai',
                'llm_model' => 'gpt-5.4',
            ],
        );

        $generation->update([
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::INTERPRETING,
            'generator_provider' => 'klass-python',
            'generator_model' => 'renderer-v1',
            'interpretation_payload' => ['schema_version' => 'media_prompt_understanding.v1'],
            'generation_spec_payload' => ['schema_version' => 'media_generation_spec.v1'],
            'delivery_payload' => ['artifact' => ['file_url' => 'https://example.com/materials/algebra.pdf']],
            'generator_service_response' => ['request_id' => 'gen-123'],
            'storage_path' => 'materials/generated/algebra.pdf',
            'file_url' => 'https://example.com/materials/algebra.pdf',
            'thumbnail_url' => 'https://example.com/materials/algebra-thumb.jpg',
            'mime_type' => 'application/pdf',
            'error_code' => 'temporary_warning',
            'error_message' => 'Retry still allowed.',
        ]);

        $generation->refresh();

        $this->assertSame($teacher->id, $generation->teacher_id);
        $this->assertSame($subject->id, $generation->subject_id);
        $this->assertSame($subSubject->id, $generation->sub_subject_id);
        $this->assertSame('pdf', $generation->preferred_output_type);
        $this->assertSame('pdf', $generation->resolved_output_type);
        $this->assertSame('openai', $generation->llm_provider);
        $this->assertSame('gpt-5.4', $generation->llm_model);
        $this->assertSame('klass-python', $generation->generator_provider);
        $this->assertSame('renderer-v1', $generation->generator_model);
        $this->assertSame('materials/generated/algebra.pdf', $generation->storage_path);
        $this->assertSame('application/pdf', $generation->mime_type);
        $this->assertSame('gen-123', data_get($generation->generator_service_response, 'request_id'));
        $this->assertSame('media-generation:' . $generation->id, $generation->jobKey());
        $this->assertNotNull($generation->request_fingerprint);
        $this->assertSame($generation->request_fingerprint, $generation->active_duplicate_key);

        $reusedGeneration = $service->createOrReuse(
            teacherId: $teacher->id,
            rawPrompt: 'Buatkan handout aljabar dasar untuk siswa SMP.',
            preferredOutputType: 'pdf',
            subjectId: $subject->id,
            subSubjectId: $subSubject->id,
        );

        $this->assertSame($generation->id, $reusedGeneration->id);
        $this->assertDatabaseCount('media_generations', 1);

        $generation->update([
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);
        $generation->refresh();

        $this->assertNull($generation->active_duplicate_key);
        $this->assertTrue($generation->isTerminal());

        $newGeneration = $service->createOrReuse(
            teacherId: $teacher->id,
            rawPrompt: 'Buatkan handout aljabar dasar untuk siswa SMP.',
            preferredOutputType: 'pdf',
            subjectId: $subject->id,
            subSubjectId: $subSubject->id,
        );

        $this->assertNotSame($generation->id, $newGeneration->id);
        $this->assertDatabaseCount('media_generations', 2);
        $this->assertSame($generation->request_fingerprint, $newGeneration->request_fingerprint);
    }

    public function test_media_publication_service_publishes_workspace_and_feed_entities_idempotently(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subject = Subject::query()->where('slug', 'science')->firstOrFail();
        $subSubject = SubSubject::query()->where('slug', 'thermodynamics')->firstOrFail();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subject->id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan slide deck termodinamika untuk kelas 11 dengan latihan singkat.',
            'preferred_output_type' => 'pptx',
            'resolved_output_type' => 'pptx',
            'status' => MediaGenerationLifecycle::PUBLISHING,
            'file_url' => 'https://example.com/materials/thermodynamics-deck.pptx',
            'thumbnail_url' => 'https://example.com/materials/thermodynamics-deck.jpg',
            'storage_path' => 'materials/generated/thermodynamics-deck.pptx',
            'mime_type' => 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
            'interpretation_payload' => [
                'teacher_delivery_summary' => 'Gunakan deck ini untuk membuka diskusi kelas dan latihan konsep kalor.',
                'confidence' => ['score' => 0.91],
                'document_blueprint' => [
                    'title' => 'Deck Termodinamika Kelas 11',
                    'sections' => [
                        ['title' => 'Konsep Dasar'],
                        ['title' => 'Latihan Cepat'],
                    ],
                ],
            ],
            'generation_spec_payload' => [
                'title' => 'Deck Termodinamika Kelas 11',
                'summary' => 'Slide pembuka materi termodinamika dengan latihan singkat.',
                'export_format' => 'pptx',
                'sections' => [
                    ['title' => 'Konsep Dasar'],
                    ['title' => 'Latihan Cepat'],
                ],
            ],
        ]);

        $service = new MediaPublicationService();
        $publishedGeneration = $service->publish($generation);

        $topic = Topic::query()->findOrFail($publishedGeneration->topic_id);
        $content = Content::query()->findOrFail($publishedGeneration->content_id);
        $project = RecommendedProject::query()->findOrFail($publishedGeneration->recommended_project_id);

        $this->assertSame('Deck Termodinamika Kelas 11', $topic->title);
        $this->assertSame((string) $teacher->id, $topic->teacher_id);
        $this->assertSame($teacher->id, $topic->owner_user_id);
        $this->assertSame($subSubject->id, $topic->sub_subject_id);
        $this->assertSame('brief', $content->type);
        $this->assertSame('https://example.com/materials/thermodynamics-deck.pptx', $content->media_url);
        $this->assertSame($generation->id, data_get($content->data, 'media_generation_id'));
        $this->assertSame(RecommendedProject::SOURCE_AI_GENERATED, $project->source_type);
        $this->assertSame((string) $project->id, $project->source_reference);
        $this->assertSame('https://example.com/materials/thermodynamics-deck.pptx', $project->project_file_url);
        $this->assertSame($generation->id, data_get($project->source_payload, 'media_generation_id'));
        $this->assertSame($topic->id, data_get($project->source_payload, 'topic_id'));
        $this->assertSame($content->id, data_get($project->source_payload, 'content_id'));
        $this->assertSame($subject->id, data_get($project->source_payload, 'subject_id'));
        $this->assertSame($subSubject->id, data_get($project->source_payload, 'sub_subject_id'));
        $this->assertSame('pptx', data_get($project->source_payload, 'output_type'));
        $this->assertSame('Deck Termodinamika Kelas 11', data_get($publishedGeneration->delivery_payload, 'publication.topic.title'));
        $this->assertSame((string) $project->id, data_get($publishedGeneration->delivery_payload, 'publication.recommended_project.id'));

        $republishedGeneration = $service->publish($publishedGeneration);

        $this->assertSame($topic->id, $republishedGeneration->topic_id);
        $this->assertSame($content->id, $republishedGeneration->content_id);
        $this->assertSame($project->id, $republishedGeneration->recommended_project_id);
        $this->assertSame(1, Topic::query()->count());
        $this->assertSame(1, Content::query()->count());
        $this->assertSame(1, RecommendedProject::query()->where('source_type', RecommendedProject::SOURCE_AI_GENERATED)->count());

        $feed = (new RecommendationAggregationService())->buildFeed();
        $aiGeneratedItem = $feed->firstWhere('id', (string) $project->id);
        $rawSystemTopicItem = $feed
            ->where('source_type', RecommendedProject::SOURCE_SYSTEM_TOPIC)
            ->firstWhere('source_reference', $topic->id);

        $this->assertNotNull($aiGeneratedItem);
        $this->assertSame($project->title, $aiGeneratedItem['title']);
        $this->assertSame($subSubject->id, $aiGeneratedItem['sub_subject_id']);
        $this->assertNull($rawSystemTopicItem);
    }

    public function test_media_publication_service_can_use_inferred_taxonomy_from_interpretation_audit_payload(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $teacher = User::factory()->teacher()->create();
        $subject = Subject::query()->where('slug', 'ipas-sd')->firstOrFail();
        $subSubject = SubSubject::query()->where('slug', 'gaya-sekitar-kita-kelas-4')->firstOrFail();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan PDF pembelajaran IPAS kelas 4 tentang Gaya di Sekitar Kita.',
            'preferred_output_type' => 'pdf',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::PUBLISHING,
            'file_url' => 'https://example.com/materials/ipas-gaya-kelas-4.pdf',
            'thumbnail_url' => 'https://example.com/materials/ipas-gaya-kelas-4.jpg',
            'storage_path' => 'materials/generated/ipas-gaya-kelas-4.pdf',
            'mime_type' => 'application/pdf',
            'interpretation_payload' => [
                'teacher_delivery_summary' => 'Gunakan materi ini untuk membahas pengaruh gaya terhadap benda.',
                'subject_context' => [
                    'subject_name' => 'Ilmu Pengetahuan Alam dan Sosial (IPAS)',
                    'subject_slug' => 'ipas-sd',
                ],
                'sub_subject_context' => [
                    'sub_subject_name' => 'Gaya di Sekitar Kita',
                    'sub_subject_slug' => 'gaya-sekitar-kita-kelas-4',
                ],
                'document_blueprint' => [
                    'title' => 'Materi Gaya di Sekitar Kita',
                    'sections' => [
                        ['title' => 'Konsep Inti'],
                        ['title' => 'Contoh Fenomena'],
                    ],
                ],
            ],
            'interpretation_audit_payload' => [
                'taxonomy_inference' => [
                    'best_match' => [
                        'subject_id' => $subject->id,
                        'sub_subject_id' => $subSubject->id,
                    ],
                ],
            ],
            'generation_spec_payload' => [
                'title' => 'Materi Gaya di Sekitar Kita',
                'summary' => 'Ringkasan pembelajaran gaya untuk siswa kelas 4 SD.',
                'export_format' => 'pdf',
                'sections' => [
                    ['title' => 'Konsep Inti'],
                    ['title' => 'Contoh Fenomena'],
                ],
            ],
        ]);

        $publishedGeneration = (new MediaPublicationService())->publish($generation);
        $topic = Topic::query()->findOrFail($publishedGeneration->topic_id);
        $project = RecommendedProject::query()->findOrFail($publishedGeneration->recommended_project_id);

        $this->assertSame($subSubject->id, $topic->sub_subject_id);
        $this->assertSame($subject->id, data_get($project->source_payload, 'subject_id'));
        $this->assertSame($subSubject->id, data_get($project->source_payload, 'sub_subject_id'));
        $this->assertSame($subSubject->id, data_get($project->source_payload, 'taxonomy.sub_subject.id'));
    }

    public function test_file_upload_service_generates_public_url_from_runtime_safe_config(): void
    {
        config([
            'filesystems.disks.supabase.bucket' => 'klass-storage',
            'filesystems.disks.supabase.public_base_url' => 'https://zcpfiryeowxsmvcqhssh.supabase.co',
            'filesystems.disks.supabase.endpoint' => 'https://zcpfiryeowxsmvcqhssh.supabase.co/storage/v1/s3',
        ]);

        $url = (new FileUploadService())->generatePublicUrl('materials/generated/algebra.pdf');

        $this->assertSame(
            'https://zcpfiryeowxsmvcqhssh.supabase.co/storage/v1/object/public/klass-storage/materials/generated/algebra.pdf',
            $url
        );
    }

    public function test_file_upload_service_derives_public_url_from_storage_endpoint_when_project_base_url_is_unavailable(): void
    {
        config([
            'filesystems.disks.supabase.bucket' => 'klass-storage',
            'filesystems.disks.supabase.public_base_url' => null,
            'filesystems.disks.supabase.endpoint' => 'https://zcpfiryeowxsmvcqhssh.supabase.co/storage/v1/s3',
        ]);

        $url = (new FileUploadService())->generatePublicUrl('gallery/generated/thumb.svg');

        $this->assertSame(
            'https://zcpfiryeowxsmvcqhssh.supabase.co/storage/v1/object/public/klass-storage/gallery/generated/thumb.svg',
            $url
        );
    }
}