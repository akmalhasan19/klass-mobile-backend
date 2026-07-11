<?php

namespace Tests\Unit;

use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use Tests\TestCase;

class MediaPromptInterpretationSchemaTest extends TestCase
{
    public function test_decode_and_validate_normalizes_candidates_and_preserves_schema_shape(): void
    {
        $payload = MediaPromptInterpretationSchema::decodeAndValidate(
            json_encode($this->validPayload(), JSON_THROW_ON_ERROR)
        );

        $this->assertSame(MediaPromptInterpretationSchema::VERSION, $payload['schema_version']);
        $this->assertSame(['pdf', 'docx', 'pptx'], array_column($payload['output_type_candidates'], 'type'));
        $this->assertSame(0.82, $payload['output_type_candidates'][0]['score']);
        $this->assertSame('medium', $payload['requested_media_characteristics']['visual_density']);
        $this->assertFalse($payload['fallback']['triggered']);
    }

    public function test_decode_and_validate_rejects_markdown_wrapped_json(): void
    {
        $this->expectException(MediaGenerationContractException::class);

        MediaPromptInterpretationSchema::decodeAndValidate("```json\n{}\n```");
    }

    public function test_fallback_produces_retryable_schema_valid_payload(): void
    {
        $payload = MediaPromptInterpretationSchema::fallback(
            'Buatkan materi ringkas untuk kelas 8.',
            preferredOutputType: 'docx',
        );

        $this->assertTrue($payload['fallback']['triggered']);
        $this->assertSame('use_safe_lesson_blueprint', $payload['fallback']['action']);
        $this->assertSame('docx', $payload['constraints']['preferred_output_type']);
        $this->assertSame('docx', $payload['output_type_candidates'][0]['type']);
        $this->assertStringNotContainsString('Retry', $payload['document_blueprint']['title']);
    }

    public function test_fallback_can_follow_taxonomy_content_structure_when_hint_is_available(): void
    {
        $payload = MediaPromptInterpretationSchema::fallback(
            'Buatkan PDF pembelajaran IPAS kelas 4 tentang gaya di sekitar kita.',
            preferredOutputType: 'pdf',
            taxonomyHint: [
                'best_match' => [
                    'description' => 'Memahami konsep gaya dan pengaruhnya terhadap benda, serta mengenal berbagai jenis gaya dalam kehidupan sehari-hari.',
                    'structure_items' => ['Konsep', 'Hukum/Rumus', 'Contoh fenomena', 'Eksperimen aman bila relevan'],
                ],
            ],
        );

        $this->assertSame('pdf', $payload['constraints']['preferred_output_type']);
        $this->assertCount(4, $payload['document_blueprint']['sections']);
        $this->assertStringContainsString('Memahami konsep gaya', $payload['document_blueprint']['summary']);
        $this->assertSame('Konsep Inti Gaya Di Sekitar Kita', $payload['document_blueprint']['sections'][0]['title']);
    }

    /**
     * @return array<string, mixed>
     */
    private function validPayload(): array
    {
        return [
            'schema_version' => MediaPromptInterpretationSchema::VERSION,
            'teacher_prompt' => 'Buatkan handout printable aljabar dasar untuk kelas 8.',
            'language' => 'id',
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => 'Create a printable handout for classroom use.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => [
                'Siswa memahami bentuk aljabar sederhana.',
            ],
            'constraints' => [
                'preferred_output_type' => 'auto',
                'max_duration_minutes' => 40,
                'must_include' => ['contoh soal'],
                'avoid' => ['istilah kompleks'],
                'tone' => 'supportive',
            ],
            'output_type_candidates' => [
                [
                    'type' => 'pptx',
                    'score' => 0.29,
                    'reason' => 'Slide deck hanya alternatif sekunder.',
                ],
                [
                    'type' => 'docx',
                    'score' => 0.61,
                    'reason' => 'Dokumen editable masih mungkin dipakai guru.',
                ],
                [
                    'type' => 'pdf',
                    'score' => 0.82,
                    'reason' => 'Format printable paling cocok untuk distribusi kelas.',
                ],
            ],
            'resolved_output_type_reasoning' => 'PDF paling cocok untuk materi printable yang ingin konsisten di semua perangkat.',
            'document_blueprint' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Ringkasan singkat aljabar dasar untuk pembuka dan latihan.',
                'sections' => [
                    [
                        'title' => 'Konsep Dasar',
                        'purpose' => 'Memperkenalkan istilah aljabar inti.',
                        'bullets' => ['Variabel', 'Ekspresi sederhana'],
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
                'format_preferences' => ['printable', 'structured'],
                'visual_density' => 'medium',
            ],
            'assets' => [],
            'assessment_or_activity_blocks' => [],
            'teacher_delivery_summary' => 'Gunakan handout ini untuk pengantar lalu lanjutkan ke latihan.',
            'confidence' => [
                'score' => 0.91,
                'label' => 'high',
                'rationale' => 'Prompt jelas dan langsung menyebut kebutuhan printable handout.',
            ],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }
}