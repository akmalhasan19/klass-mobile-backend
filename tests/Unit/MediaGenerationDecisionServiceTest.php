<?php

namespace Tests\Unit;

use App\Services\MediaGenerationDecisionService;
use Tests\TestCase;

class MediaGenerationDecisionServiceTest extends TestCase
{
    public function test_decide_prioritizes_teacher_override_over_candidate_ranking(): void
    {
        $decision = (new MediaGenerationDecisionService())->decide(
            $this->validInterpretationPayload(),
            'pptx',
        );

        $this->assertSame('pptx', $decision['resolved_output_type']);
        $this->assertSame('teacher_override', $decision['decision_source']);
        $this->assertSame('teacher_override', $decision['reason_code']);
        $this->assertFalse($decision['tie_breaker_applied']);
        $this->assertSame('auto', $decision['constraint_preferred_output_type']);
        $this->assertSame('pdf', $decision['ranked_candidates'][0]['type']);
    }

    public function test_decide_applies_deterministic_priority_when_top_scores_are_tied(): void
    {
        $payload = $this->validInterpretationPayload();
        $payload['teacher_prompt'] = 'Buatkan media pembelajaran kelas 8 dengan ringkasan dan latihan singkat.';
        $payload['teacher_intent']['goal'] = 'Create a short classroom resource for grade 8 students.';
        $payload['resolved_output_type_reasoning'] = 'Kedua format tertinggi sama-sama layak dan perlu tie breaker deterministik.';
        $payload['requested_media_characteristics']['format_preferences'] = [];
        $payload['document_blueprint']['title'] = 'Materi Kelas 8';
        $payload['document_blueprint']['summary'] = 'Ringkasan singkat untuk pembuka materi kelas.';
        $payload['teacher_delivery_summary'] = 'Gunakan materi ini untuk pembuka dan latihan kelas.';
        $payload['output_type_candidates'] = [
            [
                'type' => 'docx',
                'score' => 0.71,
                'reason' => 'Dokumen editable cukup cocok untuk kebutuhan adaptasi guru.',
            ],
            [
                'type' => 'pdf',
                'score' => 0.71,
                'reason' => 'Dokumen printable juga sama kuatnya untuk distribusi kelas.',
            ],
            [
                'type' => 'pptx',
                'score' => 0.41,
                'reason' => 'Slide deck kurang prioritas untuk prompt netral ini.',
            ],
        ];

        $decision = (new MediaGenerationDecisionService())->decide($payload, 'auto');

        $this->assertSame('pdf', $decision['resolved_output_type']);
        $this->assertSame('candidate_ranking', $decision['decision_source']);
        $this->assertTrue($decision['tie_breaker_applied']);
        $this->assertSame(['pdf', 'docx', 'pptx'], array_column($decision['ranked_candidates'], 'type'));
        $this->assertStringContainsString('Scores tied', $decision['reasoning']);
    }

    /**
     * @return array<string, mixed>
     */
    private function validInterpretationPayload(): array
    {
        return [
            'schema_version' => 'media_prompt_understanding.v1',
            'teacher_prompt' => 'Buatkan handout aljabar untuk kelas 8 dengan contoh singkat.',
            'language' => 'id',
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => 'Create a printable classroom handout.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => [
                'Siswa memahami konsep dasar aljabar.',
            ],
            'constraints' => [
                'preferred_output_type' => 'auto',
                'max_duration_minutes' => 45,
                'must_include' => ['contoh soal'],
                'avoid' => ['istilah terlalu teknis'],
                'tone' => 'supportive',
            ],
            'output_type_candidates' => [
                [
                    'type' => 'pdf',
                    'score' => 0.78,
                    'reason' => 'Format printable paling cocok untuk dibagikan ke kelas.',
                ],
                [
                    'type' => 'docx',
                    'score' => 0.61,
                    'reason' => 'Masih cocok jika guru ingin mengedit ulang isi dokumen.',
                ],
                [
                    'type' => 'pptx',
                    'score' => 0.22,
                    'reason' => 'Slide deck tidak menjadi kebutuhan utama prompt ini.',
                ],
            ],
            'resolved_output_type_reasoning' => 'PDF paling sesuai untuk handout yang ingin tampil konsisten saat dicetak atau dibagikan.',
            'document_blueprint' => [
                'title' => 'Handout Aljabar Kelas 8',
                'summary' => 'Ringkasan singkat aljabar dasar dengan latihan cepat.',
                'sections' => [
                    [
                        'title' => 'Konsep Dasar',
                        'purpose' => 'Memperkenalkan istilah inti aljabar.',
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
                'format_preferences' => ['printable'],
                'visual_density' => 'medium',
            ],
            'assets' => [],
            'assessment_or_activity_blocks' => [],
            'teacher_delivery_summary' => 'Gunakan handout ini untuk pembuka sebelum latihan mandiri.',
            'confidence' => [
                'score' => 0.88,
                'label' => 'high',
                'rationale' => 'Prompt cukup jelas dan langsung mengarah ke materi printable.',
            ],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }
}