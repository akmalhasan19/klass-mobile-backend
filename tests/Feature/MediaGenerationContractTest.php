<?php

namespace Tests\Feature;

use App\MediaGeneration\MediaArtifactMetadataContract;
use App\MediaGeneration\MediaContentDraftRequestContract;
use App\MediaGeneration\MediaContentDraftSchema;
use App\MediaGeneration\MediaDeliveryRequestContract;
use App\MediaGeneration\MediaDeliveryResponseSchema;
use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\MediaGeneration\MediaGenerationSpecContract;
use App\MediaGeneration\MediaPromptInterpretationRequestContract;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\Models\MediaGeneration;
use App\Models\SubSubject;
use App\Models\Subject;
use Tests\TestCase;

class MediaGenerationContractTest extends TestCase
{
    public function test_lifecycle_definition_locks_minimum_statuses_retry_behaviors_and_terminal_states(): void
    {
        $definition = MediaGenerationLifecycle::definition();

        $this->assertSame(MediaGenerationLifecycle::VERSION, $definition['version']);
        $this->assertSame(
            ['queued', 'interpreting', 'classified', 'generating', 'uploading', 'publishing', 'completed', 'failed'],
            $definition['minimum_statuses']
        );
        $this->assertTrue($definition['cancelled_prepared']);
        $this->assertSame(['completed', 'failed', 'cancelled'], $definition['terminal_states']);
        $this->assertSame('restart_from_interpreting', MediaGenerationLifecycle::retryBehavior(MediaGenerationLifecycle::FAILED));
        $this->assertSame('manual_requeue_only', MediaGenerationLifecycle::retryBehavior(MediaGenerationLifecycle::CANCELLED));
    }

    public function test_lifecycle_validates_known_transitions(): void
    {
        $this->assertTrue(MediaGenerationLifecycle::canTransition(MediaGenerationLifecycle::QUEUED, MediaGenerationLifecycle::INTERPRETING));
        $this->assertTrue(MediaGenerationLifecycle::canTransition(MediaGenerationLifecycle::PUBLISHING, MediaGenerationLifecycle::COMPLETED));
        $this->assertFalse(MediaGenerationLifecycle::canTransition(MediaGenerationLifecycle::COMPLETED, MediaGenerationLifecycle::GENERATING));
        $this->assertFalse(MediaGenerationLifecycle::canTransition(MediaGenerationLifecycle::PUBLISHING, MediaGenerationLifecycle::CANCELLED));
    }

    public function test_prompt_interpretation_schema_decodes_json_only_payload_and_sorts_candidates(): void
    {
        $payload = MediaPromptInterpretationSchema::decodeAndValidate(json_encode($this->validInterpretationPayload(), JSON_THROW_ON_ERROR));

        $this->assertSame(MediaPromptInterpretationSchema::VERSION, $payload['schema_version']);
        $this->assertSame('pdf', $payload['output_type_candidates'][0]['type']);
        $this->assertSame('docx', $payload['output_type_candidates'][1]['type']);
        $this->assertFalse($payload['fallback']['triggered']);
        $this->assertStringContainsString('Return exactly one JSON object.', MediaPromptInterpretationSchema::llmInstruction());
    }

    public function test_prompt_interpretation_schema_rejects_invalid_json_only_contract(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        MediaPromptInterpretationSchema::decodeAndValidate("```json\n{}\n```");
    }

    public function test_prompt_interpretation_schema_builds_deterministic_fallback_payload(): void
    {
        $fallback = MediaPromptInterpretationSchema::fallback(
            'Buatkan media belajar pecahan untuk kelas 5.',
            preferredOutputType: 'pdf'
        );

        $this->assertTrue($fallback['fallback']['triggered']);
        $this->assertSame('pdf', $fallback['constraints']['preferred_output_type']);
        $this->assertSame('pdf', $fallback['output_type_candidates'][0]['type']);
        $this->assertSame('use_safe_lesson_blueprint', $fallback['fallback']['action']);
        $this->assertStringContainsString('Pecahan', $fallback['document_blueprint']['title']);
        $this->assertStringNotContainsString('Retry', $fallback['document_blueprint']['title']);
        $this->assertCount(4, $fallback['document_blueprint']['sections']);
    }

    public function test_prompt_interpretation_schema_rejects_internal_prompt_language_in_blueprint(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        $payload = $this->validInterpretationPayload();
        $payload['document_blueprint']['summary'] = 'Return exactly one JSON object and use schema_version media_prompt_understanding.v1.';

        MediaPromptInterpretationSchema::validate($payload);
    }

    public function test_prompt_interpretation_request_contract_builds_stable_adapter_payload(): void
    {
        $subject = new Subject([
            'name' => 'Matematika',
            'slug' => 'matematika',
        ]);
        $subject->id = 10;

        $subSubject = new SubSubject([
            'subject_id' => $subject->id,
            'name' => 'Pecahan',
            'slug' => 'pecahan',
        ]);
        $subSubject->id = 11;
        $subSubject->setRelation('subject', $subject);

        $generation = new MediaGeneration([
            'raw_prompt' => 'Buatkan handout pecahan untuk kelas 5.',
            'preferred_output_type' => 'pdf',
        ]);
        $generation->id = 'gen-123';
        $generation->setRelation('subject', $subject);
        $generation->setRelation('subSubject', $subSubject);

        $payload = MediaPromptInterpretationRequestContract::fromGeneration(
            $generation,
            'gpt-5.4',
            'Return exactly one JSON object.'
        );

        $this->assertSame([
            'request_type' => MediaPromptInterpretationRequestContract::REQUEST_TYPE,
            'generation_id' => 'gen-123',
            'model' => 'gpt-5.4',
            'instruction' => 'Return exactly one JSON object.',
            'input' => [
                'teacher_prompt' => 'Buatkan handout pecahan untuk kelas 5.',
                'preferred_output_type' => 'pdf',
                'subject_context' => [
                    'id' => 10,
                    'name' => 'Matematika',
                    'slug' => 'matematika',
                ],
                'sub_subject_context' => [
                    'id' => 11,
                    'name' => 'Pecahan',
                    'slug' => 'pecahan',
                ],
            ],
        ], $payload);
    }

    public function test_prompt_interpretation_request_contract_rejects_unsupported_fields(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        MediaPromptInterpretationRequestContract::validate([
            'request_type' => MediaPromptInterpretationRequestContract::REQUEST_TYPE,
            'generation_id' => 'gen-123',
            'model' => 'gpt-5.4',
            'instruction' => 'Return exactly one JSON object.',
            'input' => [
                'teacher_prompt' => 'Buatkan handout pecahan untuk kelas 5.',
                'preferred_output_type' => 'pdf',
                'subject_context' => null,
                'sub_subject_context' => null,
                'binary' => 'forbidden',
            ],
        ]);
    }

    public function test_delivery_request_contract_builds_metadata_only_payload(): void
    {
        $generation = new MediaGeneration([
            'interpretation_payload' => [
                'teacher_delivery_summary' => 'Bagikan file setelah pengantar singkat.',
            ],
            'generation_spec_payload' => [
                'teacher_delivery_summary' => 'Gunakan handout ini untuk menjelaskan konsep inti lalu lanjutkan ke latihan singkat.',
                'summary' => 'Handout untuk penguatan konsep pecahan.',
            ],
        ]);
        $generation->id = 'gen-456';

        $payload = MediaDeliveryRequestContract::fromGeneration(
            $generation,
            [
                'artifact' => [
                    'output_type' => 'pdf',
                    'title' => 'Handout Pecahan Kelas 5',
                    'file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                    'thumbnail_url' => 'https://example.com/gallery/handout-pecahan-kelas-5.svg',
                    'mime_type' => 'application/pdf',
                    'filename' => 'handout-pecahan-kelas-5.pdf',
                ],
                'publication' => [
                    'topic' => ['id' => 'topic-123', 'title' => 'Handout Pecahan Kelas 5'],
                    'content' => ['id' => 'content-123', 'title' => 'Handout Pecahan Kelas 5', 'type' => 'brief', 'media_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf'],
                    'recommended_project' => ['id' => 'project-123', 'title' => 'Handout Pecahan Kelas 5', 'project_file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf'],
                ],
                'preview_summary' => 'Handout siap dipakai untuk penguatan konsep dan latihan singkat.',
            ],
            'gpt-5.4',
            'Return exactly one JSON object.'
        );

        $this->assertSame(MediaDeliveryRequestContract::REQUEST_TYPE, $payload['request_type']);
        $this->assertSame('gen-456', $payload['generation_id']);
        $this->assertSame('Gunakan handout ini untuk menjelaskan konsep inti lalu lanjutkan ke latihan singkat.', $payload['input']['teacher_delivery_summary']);
        $this->assertSame('Handout untuk penguatan konsep pecahan.', $payload['input']['generation_summary']);
        $this->assertArrayNotHasKey('binary', $payload['input']['artifact']);
        $this->assertArrayNotHasKey('base64', $payload['input']['artifact']);
    }

    public function test_content_draft_schema_validates_full_material_contract_and_instruction(): void
    {
        $payload = MediaContentDraftSchema::decodeAndValidate(json_encode($this->validContentDraftPayload(), JSON_THROW_ON_ERROR));

        $this->assertSame(MediaContentDraftSchema::VERSION, $payload['schema_version']);
        $this->assertSame('paragraph', $payload['sections'][0]['body_blocks'][0]['type']);
        $this->assertFalse($payload['fallback']['triggered']);
        $this->assertStringContainsString('Write actual teaching content', MediaContentDraftSchema::llmInstruction());
        $this->assertStringContainsString('Do not write outline scaffolding', MediaContentDraftSchema::llmInstruction());
    }

    public function test_content_draft_schema_can_build_deterministic_fallback_from_interpretation(): void
    {
        $payload = MediaContentDraftSchema::fallbackFromInterpretation(
            $this->validInterpretationPayload(),
            'pdf',
            'drafting_service_unconfigured',
        );

        $this->assertSame(MediaContentDraftSchema::VERSION, $payload['schema_version']);
        $this->assertTrue($payload['fallback']['triggered']);
        $this->assertSame('drafting_service_unconfigured', $payload['fallback']['reason_code']);
        $this->assertSame('paragraph', $payload['sections'][0]['body_blocks'][0]['type']);
        $this->assertNotEmpty($payload['sections'][0]['body_blocks'][0]['content']);
        $this->assertSame('paragraph', $payload['sections'][0]['body_blocks'][1]['type']);
        $this->assertSame('use_safe_lesson_fallback', $payload['fallback']['action']);
        $this->assertStringNotContainsString('Retry', $payload['sections'][0]['body_blocks'][0]['content']);
        $this->assertStringNotContainsString('Bagian ini disusun untuk', $payload['sections'][0]['body_blocks'][0]['content']);
        $this->assertStringNotContainsString('Fokus utamanya meliputi', $payload['sections'][0]['body_blocks'][0]['content']);
        $this->assertStringNotContainsString('Jelaskan ide pokoknya secara runtut', $payload['sections'][0]['body_blocks'][0]['content']);
    }

    public function test_content_draft_schema_rejects_internal_prompt_language_in_body_blocks(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        $payload = $this->validContentDraftPayload();
        $payload['sections'][0]['body_blocks'][0]['content'] = 'Return exactly one JSON object. Use schema_version media_content_draft.v1.';

        MediaContentDraftSchema::validate($payload, 'pdf');
    }

    public function test_content_draft_schema_rejects_outline_scaffolding_in_body_blocks(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        $payload = $this->validContentDraftPayload();
        $payload['sections'][0]['body_blocks'][0]['content'] = 'Bagian ini disusun untuk siswa kelas 5. Fokus utamanya meliputi pecahan senilai. Jelaskan ide pokoknya secara runtut agar siswa memahami konsep.';

        MediaContentDraftSchema::validate($payload, 'pdf');
    }

    public function test_content_draft_schema_requires_explanatory_paragraph_for_document_output(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        $payload = $this->validContentDraftPayload();
        $payload['sections'][0]['body_blocks'] = [
            [
                'type' => 'bullet',
                'content' => 'Hanya poin singkat tanpa paragraf penjelasan yang siap dibaca.',
            ],
        ];

        MediaContentDraftSchema::validate($payload, 'pdf');
    }

    public function test_content_draft_request_contract_builds_adapter_payload_from_generation_interpretation(): void
    {
        $generation = new MediaGeneration([
            'interpretation_payload' => $this->validInterpretationPayload(),
        ]);
        $generation->id = 'gen-draft-456';

        $payload = MediaContentDraftRequestContract::fromGeneration(
            $generation,
            ['resolved_output_type' => 'pdf'],
            'gpt-5.4',
            'Return exactly one JSON object.'
        );

        $this->assertSame(MediaContentDraftRequestContract::REQUEST_TYPE, $payload['request_type']);
        $this->assertSame('gen-draft-456', $payload['generation_id']);
        $this->assertSame('pdf', $payload['input']['resolved_output_type']);
        $this->assertSame(MediaPromptInterpretationSchema::VERSION, data_get($payload, 'input.interpretation.schema_version'));
        $this->assertSame('interpretation_context', data_get($payload, 'input.taxonomy_hint.source'));
        $this->assertSame('Matematika', data_get($payload, 'input.taxonomy_hint.subject.name'));
        $this->assertSame('Pecahan', data_get($payload, 'input.taxonomy_hint.sub_subject.name'));
    }

    public function test_delivery_request_contract_rejects_binary_fields(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        $payload = $this->validDeliveryRequestPayload();
        $payload['input']['artifact']['binary'] = 'forbidden';

        MediaDeliveryRequestContract::validate($payload);
    }

    public function test_delivery_response_schema_validates_supported_payload_and_json_only_instruction(): void
    {
        $payload = MediaDeliveryResponseSchema::validate($this->validDeliveryResponsePayload());

        $this->assertSame(MediaDeliveryResponseSchema::VERSION, $payload['schema_version']);
        $this->assertFalse($payload['fallback']['triggered']);
        $this->assertStringContainsString('Return exactly one JSON object.', MediaDeliveryResponseSchema::llmInstruction());
        $this->assertStringContainsString('Do not include any raw binary, base64, or attachment bytes.', MediaDeliveryResponseSchema::llmInstruction());
    }

    public function test_delivery_response_schema_rejects_unsupported_attachment_fields(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        $payload = $this->validDeliveryResponsePayload();
        $payload['artifact']['binary'] = 'forbidden';

        MediaDeliveryResponseSchema::validate($payload);
    }

    public function test_generation_spec_contract_normalizes_interpretation_payload_without_raw_prompt(): void
    {
        $spec = MediaGenerationSpecContract::fromInterpretation($this->validInterpretationPayload());

        $this->assertSame(MediaGenerationSpecContract::VERSION, $spec['schema_version']);
        $this->assertSame('pdf', $spec['export_format']);
        $this->assertArrayNotHasKey('teacher_prompt', $spec);
        $this->assertSame('document', $spec['layout_hints']['document_mode']);
        $this->assertSame(MediaArtifactMetadataContract::VERSION, $spec['contract_versions']['generator_output_metadata']);
        $this->assertSame('bullet', $spec['sections'][0]['body_blocks'][0]['type']);
    }

    public function test_generation_spec_contract_honors_override_and_validates_python_metadata(): void
    {
        $spec = MediaGenerationSpecContract::fromInterpretation($this->validInterpretationPayload(), 'pptx');

        $this->assertSame('pptx', $spec['export_format']);
        $this->assertSame('slide', $spec['page_or_slide_structure']['unit_type']);

        $metadata = MediaArtifactMetadataContract::validate([
            'schema_version' => MediaArtifactMetadataContract::VERSION,
            'export_format' => 'pptx',
            'title' => 'Deck Pecahan',
            'filename' => 'deck-pecahan.pptx',
            'extension' => 'pptx',
            'mime_type' => 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
            'size_bytes' => 24576,
            'checksum_sha256' => str_repeat('a', 64),
            'slide_count' => 12,
            'artifact_locator' => [
                'kind' => 'temporary_path',
                'value' => '/tmp/deck-pecahan.pptx',
            ],
            'generator' => [
                'name' => 'klass-media-generator',
                'version' => '0.1.0',
            ],
        ]);

        $this->assertSame('pptx', $metadata['extension']);
        $this->assertSame(12, $metadata['slide_count']);
    }

    public function test_generation_spec_contract_can_build_from_full_content_draft(): void
    {
        $spec = MediaGenerationSpecContract::fromDraft(
            $this->validInterpretationPayload(),
            $this->validContentDraftPayload(),
            'pdf',
        );

        $this->assertSame('pdf', $spec['export_format']);
        $this->assertSame('Handout Pecahan Kelas 5', $spec['title']);
        $this->assertSame('paragraph', $spec['sections'][0]['body_blocks'][0]['type']);
        $this->assertStringContainsString('Pecahan senilai adalah', $spec['sections'][0]['body_blocks'][0]['content']);
        $this->assertSame(
            'Gunakan handout ini untuk membangun pemahaman konsep sebelum siswa mengerjakan latihan mandiri.',
            $spec['teacher_delivery_summary']
        );
    }

    public function test_python_metadata_contract_rejects_mismatched_extension(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        MediaArtifactMetadataContract::validate([
            'schema_version' => MediaArtifactMetadataContract::VERSION,
            'export_format' => 'pdf',
            'title' => 'Handout Pecahan',
            'filename' => 'handout-pecahan.docx',
            'extension' => 'docx',
            'mime_type' => 'application/pdf',
            'size_bytes' => 12000,
            'checksum_sha256' => str_repeat('b', 64),
            'artifact_locator' => [
                'kind' => 'temporary_path',
                'value' => '/tmp/handout-pecahan.pdf',
            ],
            'generator' => [
                'name' => 'klass-media-generator',
                'version' => '0.1.0',
            ],
        ]);
    }

    public function test_python_metadata_contract_rejects_filename_extension_mismatch(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        MediaArtifactMetadataContract::validate([
            'schema_version' => MediaArtifactMetadataContract::VERSION,
            'export_format' => 'pdf',
            'title' => 'Handout Pecahan',
            'filename' => 'handout-pecahan.docx',
            'extension' => 'pdf',
            'mime_type' => 'application/pdf',
            'size_bytes' => 12000,
            'checksum_sha256' => str_repeat('b', 64),
            'artifact_locator' => [
                'kind' => 'temporary_path',
                'value' => '/tmp/handout-pecahan.pdf',
            ],
            'generator' => [
                'name' => 'klass-media-generator',
                'version' => '0.1.0',
            ],
        ]);
    }

    public function test_python_metadata_contract_rejects_non_canonical_mime_type(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        MediaArtifactMetadataContract::validate([
            'schema_version' => MediaArtifactMetadataContract::VERSION,
            'export_format' => 'docx',
            'title' => 'Handout Pecahan',
            'filename' => 'handout-pecahan.docx',
            'extension' => 'docx',
            'mime_type' => 'application/zip',
            'size_bytes' => 12000,
            'checksum_sha256' => str_repeat('c', 64),
            'artifact_locator' => [
                'kind' => 'temporary_path',
                'value' => '/tmp/handout-pecahan.docx',
            ],
            'generator' => [
                'name' => 'klass-media-generator',
                'version' => '0.1.0',
            ],
        ]);
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

    private function validDeliveryRequestPayload(): array
    {
        return [
            'request_type' => MediaDeliveryRequestContract::REQUEST_TYPE,
            'generation_id' => 'gen-456',
            'model' => 'gpt-5.4',
            'instruction' => 'Return exactly one JSON object.',
            'input' => [
                'artifact' => [
                    'output_type' => 'pdf',
                    'title' => 'Handout Pecahan Kelas 5',
                    'file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                    'thumbnail_url' => 'https://example.com/gallery/handout-pecahan-kelas-5.svg',
                    'mime_type' => 'application/pdf',
                    'filename' => 'handout-pecahan-kelas-5.pdf',
                ],
                'publication' => [
                    'topic' => ['id' => 'topic-123', 'title' => 'Handout Pecahan Kelas 5'],
                    'content' => ['id' => 'content-123', 'title' => 'Handout Pecahan Kelas 5', 'type' => 'brief', 'media_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf'],
                    'recommended_project' => ['id' => 'project-123', 'title' => 'Handout Pecahan Kelas 5', 'project_file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf'],
                ],
                'preview_summary' => 'Handout siap dipakai untuk penguatan konsep dan latihan singkat.',
                'teacher_delivery_summary' => 'Bagikan file setelah pengantar singkat.',
                'generation_summary' => 'Handout untuk penguatan konsep pecahan.',
            ],
        ];
    }

    private function validDeliveryResponsePayload(): array
    {
        return [
            'schema_version' => MediaDeliveryResponseSchema::VERSION,
            'title' => 'Handout Pecahan Kelas 5 siap digunakan',
            'preview_summary' => 'Handout ini cocok untuk penguatan konsep dan latihan singkat di kelas.',
            'teacher_message' => 'Materi sudah siap dipakai. Tinjau bagian contoh soal sebelum dibagikan ke siswa.',
            'recommended_next_steps' => [
                'Baca cepat struktur materi sebelum kelas dimulai.',
                'Bagikan file ke siswa setelah pengantar singkat.',
            ],
            'classroom_tips' => [
                'Mulai dengan contoh sederhana sebelum latihan mandiri.',
            ],
            'artifact' => [
                'output_type' => 'pdf',
                'title' => 'Handout Pecahan Kelas 5',
                'file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                'thumbnail_url' => 'https://example.com/gallery/handout-pecahan-kelas-5.svg',
                'mime_type' => 'application/pdf',
                'filename' => 'handout-pecahan-kelas-5.pdf',
            ],
            'publication' => [
                'topic' => ['id' => 'topic-123', 'title' => 'Handout Pecahan Kelas 5'],
                'content' => ['id' => 'content-123', 'title' => 'Handout Pecahan Kelas 5', 'type' => 'brief', 'media_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf'],
                'recommended_project' => ['id' => 'project-123', 'title' => 'Handout Pecahan Kelas 5', 'project_file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf'],
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
        ];
    }
}