<?php

namespace App\MediaGeneration;

class PedagogicalContentClassifier
{
    private array $kurikulumStructure;

    public function __construct()
    {
        $path = config('content_integrity.kurikulum_merdeka_reference');
        if ($path && file_exists($path)) {
            $this->kurikulumStructure = json_decode(file_get_contents($path), true) ?? [];
        } else {
            $this->kurikulumStructure = [];
        }
    }

    public function classify(array $spec): array
    {
        return [
            'content_types' => $this->detectContentTypes($spec),
            'pedagogical_alignment_score' => $this->calculateStructuralAlignment($spec),
            'tone_classification' => $this->classifyTone($spec),
            'expected_structure_match' => $this->calculateStructuralAlignment($spec),
        ];
    }

    private function detectContentTypes(array $spec): array
    {
        $types = [];
        $content = '';
        
        if (isset($spec['sections']) && is_array($spec['sections'])) {
            foreach ($spec['sections'] as $sec) {
                $content .= ' ' . ($sec['purpose'] ?? '');
                if (isset($sec['body_blocks']) && is_array($sec['body_blocks'])) {
                    foreach ($sec['body_blocks'] as $bb) {
                        $content .= ' ' . ($bb['content'] ?? '');
                    }
                }
            }
        }
        
        if (isset($spec['assessment_or_activity_blocks']) && is_array($spec['assessment_or_activity_blocks'])) {
            foreach ($spec['assessment_or_activity_blocks'] as $act) {
                $content .= ' ' . ($act['instructions'] ?? '');
            }
        }
        
        $content = strtolower($content);
        
        $types['definition'] = (bool) preg_match('/\b(definisi|pengertian|adalah|merupakan|merujuk pada)\b/i', $content);
        $types['worked_example'] = (bool) preg_match('/\b(contoh|misalnya|sebagai contoh|contoh soal|penyelesaian)\b/i', $content);
        $types['exercise'] = (bool) preg_match('/\b(latihan|kerjakan|jawablah|tugas|praktik|hitunglah)\b/i', $content);
        $types['assessment'] = (bool) preg_match('/\b(evaluasi|penilaian|uji kompetensi|ulangan)\b/i', $content);

        return $types;
    }

    private function classifyTone(array $spec): string
    {
        $content = '';
        if (isset($spec['sections']) && is_array($spec['sections'])) {
            foreach ($spec['sections'] as $sec) {
                if (isset($sec['body_blocks']) && is_array($sec['body_blocks'])) {
                    foreach ($sec['body_blocks'] as $bb) {
                        $content .= ' ' . ($bb['content'] ?? '');
                    }
                }
            }
        }
        $content .= ' ' . ($spec['teacher_delivery_summary'] ?? '');
        $content = strtolower($content);
        
        // Specific first-person/perspective words as specified in 4.2
        $proceduralCount = preg_match_all('/\b(pastikan|guru harus|instruksikan|beri waktu|bagikan|langkah-langkah mengajarkan|follow)\b/i', $content);
        $conversationalCount = preg_match_all('/\b(mari kita|ayo|bagaimana kalau|coba bayangkan|hai|halo|kamu tahu tidak|here\'s)\b/i', $content);
        $academicCount = preg_match_all('/\b(berdasarkan|diketahui|hipotesis|variabel|metode|analisis|kesimpulan|provides)\b/i', $content);
        
        if ($proceduralCount > max($conversationalCount, $academicCount)) {
            return 'procedural';
        }
        
        if ($conversationalCount > $academicCount) {
            return 'conversational';
        }
        
        return 'academic';
    }

    private function calculateStructuralAlignment(array $spec): float
    {
        if (empty($this->kurikulumStructure)) {
            return 1.0;
        }
        
        $types = $this->detectContentTypes($spec);
        $presentCount = 0;
        
        $expected = ['definition', 'worked_example', 'exercise'];
        $requiredCount = count($expected);
        
        foreach ($expected as $e) {
            if (isset($types[$e]) && $types[$e] === true) {
                $presentCount++;
            }
        }
        
        if ($requiredCount > 0) {
            return (float) round($presentCount / $requiredCount, 2);
        }
        
        return 1.0;
    }
}
