<?php

namespace Tests\Unit\MediaGeneration;

use App\MediaGeneration\PedagogicalContentClassifier;
use Illuminate\Support\Facades\Config;
use Tests\TestCase;

class PedagogicalContentClassifierTest extends TestCase
{
    private PedagogicalContentClassifier $classifier;

    protected function setUp(): void
    {
        parent::setUp();
        
        // Ensure a dummy structure for testing alignment
        $tempPath = tempnam(sys_get_temp_dir(), 'kurikulum_test');
        file_put_contents($tempPath, json_encode(['dummy' => 'data']));
        Config::set('content_integrity.kurikulum_merdeka_reference', $tempPath);
        
        $this->classifier = new PedagogicalContentClassifier();
        
        // Cleanup temp file
        register_shutdown_function(fn() => @unlink($tempPath));
    }

    public function test_it_detects_content_types()
    {
        $spec = [
            'sections' => [
                [
                    'purpose' => 'Memberikan definisi tentang aljabar.',
                    'body_blocks' => [
                        ['content' => 'Aljabar adalah cabang matematika.'],
                        ['content' => 'Contoh soal: x + 2 = 5. Penyelesaiannya adalah x = 3.']
                    ]
                ],
                [
                    'purpose' => 'Latihan mandiri.',
                    'body_blocks' => [
                        ['content' => 'Kerjakan latihan berikut di buku tugas Anda.']
                    ]
                ]
            ],
            'assessment_or_activity_blocks' => [
                [
                    'instructions' => 'Lakukan evaluasi akhir bab.'
                ]
            ]
        ];

        $results = $this->classifier->classify($spec);
        $types = $results['content_types'];

        $this->assertTrue($types['definition'], 'Should detect definition');
        $this->assertTrue($types['worked_example'], 'Should detect worked example');
        $this->assertTrue($types['exercise'], 'Should detect exercise');
        $this->assertTrue($types['assessment'], 'Should detect assessment');
    }

    public function test_it_classifies_academic_tone()
    {
        $spec = [
            'sections' => [
                [
                    'body_blocks' => [
                        ['content' => 'Berdasarkan analisis variabel yang diketahui, hipotesis ini dapat divalidasi.']
                    ]
                ]
            ],
            'teacher_delivery_summary' => 'Metode ini provides hasil yang konsisten.'
        ];

        $results = $this->classifier->classify($spec);
        $this->assertEquals('academic', $results['tone_classification']);
    }

    public function test_it_classifies_conversational_tone()
    {
        $spec = [
            'sections' => [
                [
                    'body_blocks' => [
                        ['content' => 'Mari kita coba bayangkan jika kita berada di ruang angkasa. Ayo seru sekali!']
                    ]
                ]
            ]
        ];

        $results = $this->classifier->classify($spec);
        $this->assertEquals('conversational', $results['tone_classification']);
    }

    public function test_it_classifies_procedural_tone()
    {
        $spec = [
            'sections' => [
                [
                    'body_blocks' => [
                        ['content' => 'Pastikan guru harus bagikan kertas ini. Instruksikan langkah-langkah mengajarkan.']
                    ]
                ]
            ]
        ];

        $results = $this->classifier->classify($spec);
        $this->assertEquals('procedural', $results['tone_classification']);
    }

    public function test_it_calculates_structural_alignment()
    {
        // High alignment (all 3 required: definition, worked_example, exercise)
        $specHigh = [
            'sections' => [
                [
                    'body_blocks' => [
                        ['content' => 'Pengertian aljabar. Contoh soal. Kerjakan latihan.'],
                    ]
                ]
            ]
        ];
        $resultsHigh = $this->classifier->classify($specHigh);
        $this->assertEquals(1.0, $resultsHigh['pedagogical_alignment_score']);

        // Partial alignment (2 out of 3)
        $specMed = [
            'sections' => [
                [
                    'body_blocks' => [
                        ['content' => 'Definisi atom. Contoh soal atom.'],
                    ]
                ]
            ]
        ];
        $resultsMed = $this->classifier->classify($specMed);
        $this->assertEquals(0.67, $resultsMed['pedagogical_alignment_score']);

        // Low alignment (1 out of 3)
        $specLow = [
            'sections' => [
                [
                    'body_blocks' => [
                        ['content' => 'Pengertian sel.'],
                    ]
                ]
            ]
        ];
        $resultsLow = $this->classifier->classify($specLow);
        $this->assertEquals(0.33, $resultsLow['pedagogical_alignment_score']);
    }
}
