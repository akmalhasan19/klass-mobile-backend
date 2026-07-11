<?php

namespace App\Services;

use App\MediaGeneration\SubjectsJsonTaxonomyCatalog;
use App\Models\Subject;
use App\Models\SubSubject;

class MediaPromptTaxonomyInferenceService
{
    private const VERSION = 'media_prompt_taxonomy_inference.v1';

    private const MINIMUM_CONFIDENCE_SCORE = 0.25;

    private const SCORE_NORMALIZER = 24.0;

    /**
     * @var string[]
     */
    private const STOPWORDS = [
        'agar',
        'ajar',
        'aku',
        'analisis',
        'buat',
        'buatkan',
        'contoh',
        'dan',
        'dengan',
        'di',
        'dokumen',
        'file',
        'guru',
        'handout',
        'kelas',
        'kuis',
        'latihan',
        'lembar',
        'materi',
        'modul',
        'pada',
        'pdf',
        'pelajaran',
        'pembelajaran',
        'ppt',
        'pptx',
        'ringkas',
        'saya',
        'semester',
        'sebuah',
        'slide',
        'siswa',
        'soal',
        'tentang',
        'untuk',
        'yang',
    ];

    /**
     * @return array<string, mixed>|null
     */
    public function infer(string $teacherPrompt): ?array
    {
        $teacherPrompt = trim($teacherPrompt);

        if ($teacherPrompt === '') {
            return null;
        }

        $promptContext = $this->promptContext($teacherPrompt);
        $normalizedPrompt = SubjectsJsonTaxonomyCatalog::normalizeSearchText($teacherPrompt);
        $promptTokens = $this->filteredTokens($teacherPrompt);
        $rankedEntries = [];

        foreach (SubjectsJsonTaxonomyCatalog::entries() as $entry) {
            $candidate = $this->scoreEntry($entry, $normalizedPrompt, $promptTokens, $promptContext);

            if ($candidate['raw_score'] > 0.0) {
                $rankedEntries[] = $candidate;
            }
        }

        if ($rankedEntries === []) {
            return null;
        }

        usort($rankedEntries, function (array $left, array $right): int {
            if ($left['raw_score'] !== $right['raw_score']) {
                return $right['raw_score'] <=> $left['raw_score'];
            }

            return ($left['entry']['catalog_index'] ?? 0) <=> ($right['entry']['catalog_index'] ?? 0);
        });

        $bestCandidate = $rankedEntries[0];

        if ($bestCandidate['normalized_score'] < self::MINIMUM_CONFIDENCE_SCORE
            && ! $bestCandidate['signal']['subject_phrase_match']
            && ! $bestCandidate['signal']['sub_subject_phrase_match']) {
            return null;
        }

        $subjectCandidates = array_values(array_filter(
            $rankedEntries,
            static fn (array $candidate): bool => $candidate['entry']['subject_slug'] === $bestCandidate['entry']['subject_slug']
        ));
        $bestSubjectCandidate = $subjectCandidates[0];
        $includeSubSubject = $this->shouldAttachSubSubject($bestCandidate, $rankedEntries[1] ?? null);
        [$subjectModel, $subSubjectModel] = $this->resolveTaxonomyModels(
            $bestCandidate['entry']['subject_slug'],
            $includeSubSubject ? $bestCandidate['entry']['sub_subject_slug'] : null,
        );

        return [
            'schema_version' => self::VERSION,
            'source' => basename(SubjectsJsonTaxonomyCatalog::path()),
            'prompt_context' => array_filter([
                'jenjang' => $promptContext['jenjang'] ?? null,
                'kelas' => $promptContext['kelas'] ?? null,
                'semester' => $promptContext['semester'] ?? null,
                'bab' => $promptContext['bab'] ?? null,
            ], static fn (mixed $value): bool => $value !== null),
            'confidence' => [
                'score' => $bestCandidate['normalized_score'],
                'label' => $this->confidenceLabel($bestCandidate['normalized_score']),
                'sub_subject_attached' => $includeSubSubject,
            ],
            'best_match' => [
                'jenjang' => $bestCandidate['entry']['jenjang'],
                'kelas' => $bestCandidate['entry']['kelas'],
                'semester' => $bestCandidate['entry']['semester'],
                'bab' => $bestCandidate['entry']['bab'],
                'subject_name' => $bestSubjectCandidate['entry']['subject_name'],
                'subject_slug' => $bestSubjectCandidate['entry']['subject_slug'],
                'subject_id' => $subjectModel?->id,
                'sub_subject_name' => $includeSubSubject ? $bestCandidate['entry']['sub_subject_name'] : null,
                'sub_subject_slug' => $includeSubSubject ? $bestCandidate['entry']['sub_subject_slug'] : null,
                'sub_subject_id' => $includeSubSubject ? $subSubjectModel?->id : null,
                'description' => $bestCandidate['entry']['description'],
                'content_structure' => $bestCandidate['entry']['content_structure'],
                'structure_items' => $bestCandidate['entry']['structure_items'],
                'matched_signals' => $this->matchedSignals($bestCandidate['signal']),
            ],
            'candidate_matches' => array_map(
                fn (array $candidate): array => [
                    'subject_name' => $candidate['entry']['subject_name'],
                    'subject_slug' => $candidate['entry']['subject_slug'],
                    'sub_subject_name' => $candidate['entry']['sub_subject_name'],
                    'sub_subject_slug' => $candidate['entry']['sub_subject_slug'],
                    'jenjang' => $candidate['entry']['jenjang'],
                    'kelas' => $candidate['entry']['kelas'],
                    'score' => $candidate['normalized_score'],
                    'label' => $this->confidenceLabel($candidate['normalized_score']),
                ],
                array_slice($rankedEntries, 0, 3)
            ),
        ];
    }

    /**
     * @param  array<string, mixed>  $entry
     * @param  string[]  $promptTokens
     * @param  array<string, int|string|null>  $promptContext
     * @return array<string, mixed>
     */
    private function scoreEntry(array $entry, string $normalizedPrompt, array $promptTokens, array $promptContext): array
    {
        $subjectTokens = $this->filteredTokens((string) $entry['subject_name']);
        $subSubjectTokens = $this->filteredTokens((string) $entry['sub_subject_name']);
        $descriptionTokens = $this->filteredTokens((string) $entry['description']);
        $structureTokens = [];

        foreach ((array) ($entry['structure_items'] ?? []) as $structureItem) {
            $structureTokens = array_merge($structureTokens, $this->filteredTokens((string) $structureItem));
        }

        $signal = [
            'subject_phrase_match' => $this->containsPhrase($normalizedPrompt, (string) $entry['normalized_subject']),
            'sub_subject_phrase_match' => $this->containsPhrase($normalizedPrompt, (string) $entry['normalized_sub_subject']),
            'subject_overlap' => $this->overlapCount($promptTokens, $subjectTokens),
            'sub_subject_overlap' => $this->overlapCount($promptTokens, $subSubjectTokens),
            'description_overlap' => min(4, $this->overlapCount($promptTokens, $descriptionTokens)),
            'structure_overlap' => min(3, $this->overlapCount($promptTokens, array_values(array_unique($structureTokens)))),
            'jenjang_match' => $this->matchesPromptContext($promptContext['jenjang'] ?? null, $entry['jenjang'] ?? null),
            'kelas_match' => $this->matchesPromptContext($promptContext['kelas'] ?? null, $entry['kelas'] ?? null),
            'semester_match' => $this->matchesPromptContext($promptContext['semester'] ?? null, $entry['semester'] ?? null),
            'bab_match' => $this->matchesPromptContext($promptContext['bab'] ?? null, $entry['bab'] ?? null),
        ];

        $rawScore = 0.0;

        if ($signal['subject_phrase_match']) {
            $rawScore += 7.0;
        }

        if ($signal['sub_subject_phrase_match']) {
            $rawScore += 12.0;
        }

        $rawScore += $signal['subject_overlap'] * 1.5;
        $rawScore += $signal['sub_subject_overlap'] * 2.75;
        $rawScore += $signal['description_overlap'] * 0.75;
        $rawScore += $signal['structure_overlap'] * 0.35;

        $rawScore += match ($signal['jenjang_match']) {
            true => 4.5,
            false => -6.0,
            default => 0.0,
        };
        $rawScore += match ($signal['kelas_match']) {
            true => 5.5,
            false => -7.0,
            default => 0.0,
        };
        $rawScore += match ($signal['semester_match']) {
            true => 1.5,
            false => -1.0,
            default => 0.0,
        };
        $rawScore += match ($signal['bab_match']) {
            true => 1.5,
            false => -0.75,
            default => 0.0,
        };

        if (($promptContext['kelas'] ?? null) !== null && $signal['kelas_match'] === true && $signal['subject_phrase_match']) {
            $rawScore += 1.5;
        }

        if (($promptContext['jenjang'] ?? null) !== null && $signal['jenjang_match'] === true && $signal['subject_overlap'] > 0) {
            $rawScore += 1.0;
        }

        $rawScore = max(0.0, round($rawScore, 4));

        return [
            'entry' => $entry,
            'raw_score' => $rawScore,
            'normalized_score' => $this->normalizeScore($rawScore),
            'signal' => $signal,
        ];
    }

    /**
     * @return array{0: Subject|null, 1: SubSubject|null}
     */
    private function resolveTaxonomyModels(string $subjectSlug, ?string $subSubjectSlug): array
    {
        $subject = Subject::query()->where('slug', $subjectSlug)->first();
        $subSubject = null;

        if ($subSubjectSlug !== null) {
            if ($subject !== null) {
                $subSubject = SubSubject::query()
                    ->where('subject_id', $subject->id)
                    ->where('slug', $subSubjectSlug)
                    ->first();
            }

            if ($subSubject === null) {
                $subSubject = SubSubject::query()
                    ->with('subject')
                    ->where('slug', $subSubjectSlug)
                    ->first();
                $subject ??= $subSubject?->subject;
            }
        }

        return [$subject, $subSubject];
    }

    /**
     * @param  array<string, mixed>  $bestCandidate
     * @param  array<string, mixed>|null  $runnerUp
     */
    private function shouldAttachSubSubject(array $bestCandidate, ?array $runnerUp): bool
    {
        $signal = $bestCandidate['signal'];

        if ($signal['sub_subject_phrase_match']) {
            return true;
        }

        if ($signal['sub_subject_overlap'] >= 2 || $signal['description_overlap'] >= 2) {
            return true;
        }

        if ($signal['semester_match'] === true || $signal['bab_match'] === true) {
            return true;
        }

        if (! $signal['subject_phrase_match'] && $signal['subject_overlap'] === 0) {
            return false;
        }

        if ($runnerUp === null) {
            return false;
        }

        return $bestCandidate['normalized_score'] >= 0.7
            && ($bestCandidate['raw_score'] - $runnerUp['raw_score']) >= 3.0
            && $bestCandidate['entry']['subject_slug'] !== $runnerUp['entry']['subject_slug'];
    }

    private function normalizeScore(float $rawScore): float
    {
        return round(min(1, max(0, $rawScore / self::SCORE_NORMALIZER)), 4);
    }

    private function confidenceLabel(float $score): string
    {
        return match (true) {
            $score >= 0.75 => 'high',
            $score >= 0.45 => 'medium',
            default => 'low',
        };
    }

    /**
     * @param  array<string, mixed>  $signal
     * @return string[]
     */
    private function matchedSignals(array $signal): array
    {
        $matched = [];

        if ($signal['subject_phrase_match']) {
            $matched[] = 'subject_phrase';
        }

        if ($signal['sub_subject_phrase_match']) {
            $matched[] = 'sub_subject_phrase';
        }

        if ($signal['subject_overlap'] > 0) {
            $matched[] = 'subject_tokens';
        }

        if ($signal['sub_subject_overlap'] > 0) {
            $matched[] = 'sub_subject_tokens';
        }

        if ($signal['description_overlap'] > 0) {
            $matched[] = 'description_tokens';
        }

        if ($signal['structure_overlap'] > 0) {
            $matched[] = 'content_structure';
        }

        foreach (['jenjang', 'kelas', 'semester', 'bab'] as $dimension) {
            if (($signal[$dimension . '_match'] ?? null) === true) {
                $matched[] = $dimension;
            }
        }

        return $matched;
    }

    /**
     * @return array<string, int|string|null>
     */
    private function promptContext(string $teacherPrompt): array
    {
        return [
            'jenjang' => $this->detectJenjang($teacherPrompt),
            'kelas' => $this->detectClassNumber($teacherPrompt),
            'semester' => $this->detectNumberByLabel($teacherPrompt, ['semester']),
            'bab' => $this->detectNumberByLabel($teacherPrompt, ['bab', 'chapter']),
        ];
    }

    private function detectJenjang(string $teacherPrompt): ?string
    {
        $normalized = SubjectsJsonTaxonomyCatalog::normalizeSearchText($teacherPrompt);

        return match (true) {
            preg_match('/\bsmk\b/u', $normalized) === 1,
            str_contains($normalized, 'sekolah menengah kejuruan') => 'SMK',
            preg_match('/\bsma\b/u', $normalized) === 1,
            str_contains($normalized, 'sekolah menengah atas') => 'SMA',
            preg_match('/\bsmp\b/u', $normalized) === 1,
            str_contains($normalized, 'sekolah menengah pertama') => 'SMP',
            preg_match('/\bsd\b/u', $normalized) === 1,
            str_contains($normalized, 'sekolah dasar') => 'SD',
            default => null,
        };
    }

    private function detectClassNumber(string $teacherPrompt): ?int
    {
        if (preg_match('/\b(?:kelas|grade)\s+([0-9]{1,2}|xii|xi|ix|viii|vii|vi|x|v|iv|iii|ii|i)\b/iu', $teacherPrompt, $matches) !== 1) {
            return null;
        }

        $token = strtolower(trim($matches[1]));

        if (is_numeric($token)) {
            return (int) $token;
        }

        return $this->romanNumeralToInt($token);
    }

    private function detectNumberByLabel(string $teacherPrompt, array $labels): ?int
    {
        $escapedLabels = implode('|', array_map(static fn (string $label): string => preg_quote($label, '/'), $labels));

        if (preg_match('/\b(?:' . $escapedLabels . ')\s+([0-9]{1,2})\b/iu', $teacherPrompt, $matches) !== 1) {
            return null;
        }

        return (int) $matches[1];
    }

    private function romanNumeralToInt(string $token): ?int
    {
        return match (strtoupper($token)) {
            'I' => 1,
            'II' => 2,
            'III' => 3,
            'IV' => 4,
            'V' => 5,
            'VI' => 6,
            'VII' => 7,
            'VIII' => 8,
            'IX' => 9,
            'X' => 10,
            'XI' => 11,
            'XII' => 12,
            default => null,
        };
    }

    private function containsPhrase(string $haystack, string $needle): bool
    {
        $normalizedHaystack = trim($haystack);
        $normalizedNeedle = trim($needle);

        if ($normalizedHaystack === '' || $normalizedNeedle === '') {
            return false;
        }

        return str_contains(' ' . $normalizedHaystack . ' ', ' ' . $normalizedNeedle . ' ');
    }

    /**
     * @param  string[]  $promptTokens
     * @param  string[]  $candidateTokens
     */
    private function overlapCount(array $promptTokens, array $candidateTokens): int
    {
        if ($promptTokens === [] || $candidateTokens === []) {
            return 0;
        }

        return count(array_intersect($promptTokens, array_values(array_unique($candidateTokens))));
    }

    /**
     * @return string[]
     */
    private function filteredTokens(string $text): array
    {
        $tokens = SubjectsJsonTaxonomyCatalog::tokenizeSearchText($text);

        return array_values(array_unique(array_filter($tokens, function (string $token): bool {
            if ($token === '' || ctype_digit($token)) {
                return false;
            }

            if (in_array($token, self::STOPWORDS, true)) {
                return false;
            }

            return mb_strlen($token) >= 2;
        })));
    }

    private function matchesPromptContext(mixed $promptValue, mixed $entryValue): ?bool
    {
        if ($promptValue === null || $entryValue === null || $promptValue === '') {
            return null;
        }

        return (string) $promptValue === (string) $entryValue;
    }
}