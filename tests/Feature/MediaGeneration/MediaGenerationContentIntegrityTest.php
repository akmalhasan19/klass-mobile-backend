<?php

namespace Tests\Feature\MediaGeneration;

use App\MediaGeneration\MediaGenerationSpecContract;
use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\MediaGeneration\MediaContentDraftSchema;
use App\MediaGeneration\MediaArtifactMetadataContract;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Config;
use Tests\TestCase;

class MediaGenerationContentIntegrityTest extends TestCase
{
    private array $baseInterpretation;
    private array $baseDraft;

    protected function setUp(): void
    {
        parent::setUp();
        
        $this->baseInterpretation = [
            'schema_version' => MediaPromptInterpretationSchema::VERSION,
            'teacher_prompt' => 'Buatkan materi tentang fotosintesis untuk kelas 7 SMP.',
            'language' => 'id',
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => 'Siswa memahami proses fotosintesis.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => ['Siswa dapat menjelaskan proses fotosintesis.'],
            'constraints' => [
                'preferred_output_type' => 'pdf',
                'max_duration_minutes' => 45,
                'must_include' => ['Klorofil', 'Cahaya matahari'],
                'avoid' => [],
                'tone' => 'academic',
            ],
            'output_type_candidates' => [
                ['type' => 'pdf', 'score' => 0.9, 'reason' => 'Cocok untuk handout.']
            ],
            'resolved_output_type_reasoning' => 'PDF adalah format terbaik.',
            'document_blueprint' => [
                'title' => 'Fotosintesis',
                'summary' => 'Materi tentang bagaimana tumbuhan membuat makanan.',
                'sections' => [
                    [
                        'title' => 'Pengenalan',
                        'purpose' => 'Memberikan dasar fotosintesis.',
                        'bullets' => ['Apa itu fotosintesis?'],
                        'estimated_length' => 'short',
                    ]
                ],
            ],
            'subject_context' => ['subject_name' => 'IPA', 'subject_slug' => 'ipa'],
            'sub_subject_context' => ['sub_subject_name' => 'Biologi', 'sub_subject_slug' => 'biologi'],
            'target_audience' => ['label' => 'Kelas 7 SMP', 'level' => 'middle_school'],
            'requested_media_characteristics' => [
                'tone' => 'academic',
                'format_preferences' => ['pdf'],
                'visual_density' => 'medium',
            ],
            'assets' => [],
            'assessment_or_activity_blocks' => [
                [
                    'title' => 'Kuis Pendek',
                    'type' => 'quiz',
                    'instructions' => 'Jawab pertanyaan berikut.',
                ]
            ],
            'teacher_delivery_summary' => 'Gunakan handout ini untuk diskusi kelas.',
            'confidence' => [
                'score' => 1.0,
                'label' => 'high',
                'rationale' => 'Semua informasi tersedia.',
            ],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];

        $this->baseDraft = [
            'schema_version' => MediaContentDraftSchema::VERSION,
            'title' => 'Fotosintesis',
            'summary' => 'Penjelasan lengkap fotosintesis.',
            'learning_objectives' => ['Siswa paham fotosintesis.'],
            'sections' => [
                [
                    'title' => 'Pengenalan',
                    'purpose' => 'Dasar materi.',
                    'body_blocks' => [
                        ['type' => 'paragraph', 'content' => 'Fotosintesis adalah proses tumbuhan hijau membuat makanan sendiri dengan bantuan cahaya matahari.']
                    ],
                    'emphasis' => 'medium',
                ]
            ],
            'teacher_delivery_summary' => 'Handout ini siap cetak.',
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ];
    }

    public function test_full_flow_accepts_clean_draft()
    {
        $draft = $this->baseDraft;
        $draft['content_integrity'] = [
            'integrity_score' => 0.95,
            'violations' => [],
            'classification_source' => 'adapter',
        ];

        $spec = MediaGenerationSpecContract::fromDraft($this->baseInterpretation, $draft);

        $this->assertEquals(0.95, $spec['content_integrity']['integrity_score']);
        $this->assertEmpty($spec['content_integrity']['violations']);
        $this->assertEquals('adapter', $spec['content_integrity']['classification_source']);
    }

    public function test_rejection_when_threshold_not_met_in_strict_mode()
    {
        Config::set('content_integrity.rejection_strategy', 'strict');
        Config::set('content_integrity.classifier_confidence_threshold', 0.80);

        $draft = $this->baseDraft;
        $draft['content_integrity'] = [
            'integrity_score' => 0.60,
            'violations' => [['pattern_name' => 'procedural_instruction', 'matched_text' => 'Follow these steps']],
            'classification_source' => 'adapter',
        ];

        $this->expectException(MediaGenerationContractException::class);
        $this->expectExceptionMessage('Draft content failed integrity score threshold.');

        MediaGenerationSpecContract::fromDraft($this->baseInterpretation, $draft);
    }

    public function test_warning_strategy_allows_spec_but_logs_violations()
    {
        Config::set('content_integrity.rejection_strategy', 'warn');
        Config::set('content_integrity.classifier_confidence_threshold', 0.80);

        $draft = $this->baseDraft;
        $draft['content_integrity'] = [
            'integrity_score' => 0.60,
            'violations' => [['pattern_name' => 'procedural_instruction', 'matched_text' => 'Follow these steps']],
            'classification_source' => 'adapter',
        ];

        $spec = MediaGenerationSpecContract::fromDraft($this->baseInterpretation, $draft);

        $this->assertEquals(0.60, $spec['content_integrity']['integrity_score']);
        $this->assertCount(1, $spec['content_integrity']['violations']);
        $this->assertEquals('Follow these steps', $spec['content_integrity']['violations'][0]['matched_text']);
    }

    public function test_fallback_generation_passes_integrity()
    {
        $spec = MediaGenerationSpecContract::fromInterpretation($this->baseInterpretation);

        $this->assertEquals(1.0, $spec['content_integrity']['integrity_score']);
        $this->assertEmpty($spec['content_integrity']['violations']);
        $this->assertEquals('fallback', $spec['content_integrity']['classification_source']);
        $this->assertTrue($spec['content_integrity']['metadata']['synthetic']);
    }

    public function test_violation_recording_in_spec()
    {
        $draft = $this->baseDraft;
        $violations = [
            ['pattern_name' => 'procedural_instruction', 'matched_text' => 'Step 1', 'field_path' => 'sections.0.body_blocks.0.content'],
            ['pattern_name' => 'conversational_filler', 'matched_text' => 'Here is your', 'field_path' => 'summary']
        ];
        $draft['content_integrity'] = [
            'integrity_score' => 0.50,
            'violations' => $violations,
            'classification_source' => 'adapter',
        ];

        Config::set('content_integrity.rejection_strategy', 'warn');

        $spec = MediaGenerationSpecContract::fromDraft($this->baseInterpretation, $draft);

        $this->assertCount(2, $spec['content_integrity']['violations']);
        $this->assertEquals($violations, $spec['content_integrity']['violations']);
    }
}
