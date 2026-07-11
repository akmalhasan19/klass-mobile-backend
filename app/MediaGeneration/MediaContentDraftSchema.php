<?php

namespace App\MediaGeneration;

use App\Models\MediaGeneration;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;
use JsonException;

final class MediaContentDraftSchema
{
    public const VERSION = 'media_content_draft.v1';

    public static function llmInstruction(): string
    {
        return implode("\n", [
            'Draft the full classroom-ready learning material from the interpreted teacher request.',
            'Return exactly one JSON object.',
            'Do not wrap the JSON in markdown fences.',
            'Do not add prose before or after the JSON.',
            'Use schema_version "' . self::VERSION . '".',
            'Always include these top-level keys: schema_version, title, summary, learning_objectives, sections, teacher_delivery_summary, fallback.',
            'Each sections entry must include: title, purpose, body_blocks, emphasis.',
            'Each body_blocks entry must be an object with type and content.',
            'The text in title, summary, learning_objectives, sections[].purpose, sections[].body_blocks[].content, and teacher_delivery_summary will be rendered directly into the opened file.',
            'Write actual teaching content inside body_blocks.content. Do not output planning notes, schema explanations, placeholders, or instructions about what should be written later.',
            'Write the final lesson text that teachers and students should read, not directions for another system about how to generate that lesson.',
            'When input.taxonomy_hint is present, use it to align subject naming, grade scope, terminology, and topic focus.',
            'If input.taxonomy_hint.content_guidance.structure_items is available, use those items as ordering hints when they fit the teacher request.',
            'For pdf and docx, every section must contain at least one explanatory paragraph that can be read directly as teaching material.',
            'When the topic calls for definitions, formulas, rules, worked examples, or short exercises, include them in the content instead of describing them abstractly.',
            'Prefer paragraph blocks for explanations. Use bullet or checklist blocks only for lists, steps, or short exercises.',
            'Do not write outline scaffolding such as "Bagian ini disusun untuk...", "Fokus utamanya meliputi...", or "Jelaskan ide pokoknya secara runtut...". Rewrite those ideas into final lesson prose.',
            'Use the same language as input.interpretation.language.',
            'Keep the content aligned with input.resolved_output_type: fuller prose for pdf/docx, tighter points for pptx.',
            'Never mention prompts, schema keys, JSON instructions, body_blocks, fallback flags, LLMs, adapters, renderers, or internal workflows in any teacher-facing text.',
            'Set fallback.triggered to false unless you explicitly need to signal degraded drafting.',
        ]);
    }

    public static function decodeAndValidate(string $rawJson, ?string $resolvedOutputType = null): array
    {
        $trimmed = trim($rawJson);

        if ($trimmed === '') {
            throw new MediaGenerationContractException(
                'Content draft response must not be empty.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED
            );
        }

        try {
            $decoded = json_decode($trimmed, true, 512, JSON_THROW_ON_ERROR);
        } catch (JsonException $exception) {
            throw new MediaGenerationContractException(
                'Content draft returned invalid JSON.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                ['json_error' => $exception->getMessage()]
            );
        }

        if (! is_array($decoded) || array_is_list($decoded)) {
            throw new MediaGenerationContractException(
                'Content draft must be a JSON object.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED
            );
        }

        return self::validate($decoded, $resolvedOutputType);
    }

    public static function validate(array $payload, ?string $resolvedOutputType = null): array
    {
        self::assertAllowedKeys($payload, self::topLevelKeys(), 'payload');
        self::assertNestedAllowedKeys($payload);

        $payload = self::applyDefaults($payload);

        $validator = Validator::make($payload, [
            'schema_version' => ['required', 'string', Rule::in([self::VERSION])],
            'title' => ['required', 'string', 'max:200'],
            'summary' => ['required', 'string', 'max:1000'],
            'learning_objectives' => ['present', 'array'],
            'learning_objectives.*' => ['string', 'max:300'],
            'sections' => ['required', 'array', 'min:1'],
            'sections.*.title' => ['required', 'string', 'max:200'],
            'sections.*.purpose' => ['required', 'string', 'max:500'],
            'sections.*.body_blocks' => ['required', 'array', 'min:1'],
            'sections.*.body_blocks.*.type' => ['required', 'string', Rule::in(['paragraph', 'bullet', 'checklist', 'note'])],
            'sections.*.body_blocks.*.content' => ['required', 'string', 'max:1000'],
            'sections.*.emphasis' => ['required', 'string', Rule::in(['short', 'medium', 'long'])],
            'teacher_delivery_summary' => ['required', 'string', 'max:1000'],
            'fallback' => ['required', 'array'],
            'fallback.triggered' => ['required', 'boolean'],
            'fallback.reason_code' => ['nullable', 'string', 'max:100'],
            'fallback.action' => ['nullable', 'string', 'max:100'],
        ]);

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Content draft payload failed schema validation.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                ['errors' => $validator->errors()->toArray()]
            );
        }

        $normalizedPayload = self::normalize($payload);

        MediaGeneratedContentGuard::assertContentDraftPayload($normalizedPayload, $resolvedOutputType);

        return $normalizedPayload;
    }

    public static function fallbackFromInterpretation(
        array $interpretationPayload,
        string $resolvedOutputType,
        string $reasonCode = 'content_draft_fallback'
    ): array {
        $interpretation = MediaPromptInterpretationSchema::validate($interpretationPayload);
        $normalizedOutputType = MediaGeneration::normalizePreferredOutputType($resolvedOutputType);

        return self::validate([
            'schema_version' => self::VERSION,
            'title' => $interpretation['document_blueprint']['title'],
            'summary' => $interpretation['document_blueprint']['summary'],
            'learning_objectives' => $interpretation['learning_objectives'],
            'sections' => array_map(
                static fn (array $section): array => [
                    'title' => $section['title'],
                    'purpose' => $section['purpose'],
                    'body_blocks' => self::fallbackBodyBlocks($section, $normalizedOutputType, $interpretation),
                    'emphasis' => $section['estimated_length'],
                ],
                $interpretation['document_blueprint']['sections']
            ),
            'teacher_delivery_summary' => $interpretation['teacher_delivery_summary'],
            'fallback' => [
                'triggered' => true,
                'reason_code' => $reasonCode,
                'action' => 'use_safe_lesson_fallback',
            ],
        ], $normalizedOutputType);
    }

    private static function normalize(array $payload): array
    {
        return [
            'schema_version' => self::VERSION,
            'title' => trim($payload['title']),
            'summary' => trim($payload['summary']),
            'learning_objectives' => array_values(array_map(
                static fn (string $objective): string => trim($objective),
                $payload['learning_objectives']
            )),
            'sections' => array_map(
                static fn (array $section): array => [
                    'title' => trim($section['title']),
                    'purpose' => trim($section['purpose']),
                    'body_blocks' => array_map(
                        static fn (array $block): array => [
                            'type' => trim($block['type']),
                            'content' => trim($block['content']),
                        ],
                        $section['body_blocks']
                    ),
                    'emphasis' => trim($section['emphasis']),
                ],
                $payload['sections']
            ),
            'teacher_delivery_summary' => trim($payload['teacher_delivery_summary']),
            'fallback' => [
                'triggered' => (bool) $payload['fallback']['triggered'],
                'reason_code' => $payload['fallback']['reason_code'] !== null
                    ? trim((string) $payload['fallback']['reason_code'])
                    : null,
                'action' => $payload['fallback']['action'] !== null
                    ? trim((string) $payload['fallback']['action'])
                    : null,
            ],
        ];
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private static function applyDefaults(array $payload): array
    {
        $payload = array_merge([
            'schema_version' => self::VERSION,
            'learning_objectives' => [],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ], $payload);

        if (isset($payload['fallback']) && is_array($payload['fallback'])) {
            $payload['fallback'] = array_merge([
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ], $payload['fallback']);
        }

        return $payload;
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private static function assertNestedAllowedKeys(array $payload): void
    {
        if (! isset($payload['sections']) || ! is_array($payload['sections'])) {
            return;
        }

        foreach ($payload['sections'] as $index => $section) {
            if (! is_array($section)) {
                continue;
            }

            self::assertAllowedKeys($section, ['title', 'purpose', 'body_blocks', 'emphasis'], 'sections.' . $index);

            if (! isset($section['body_blocks']) || ! is_array($section['body_blocks'])) {
                continue;
            }

            foreach ($section['body_blocks'] as $blockIndex => $block) {
                if (is_array($block)) {
                    self::assertAllowedKeys($block, ['type', 'content'], 'sections.' . $index . '.body_blocks.' . $blockIndex);
                }
            }
        }

        if (isset($payload['fallback']) && is_array($payload['fallback'])) {
            self::assertAllowedKeys($payload['fallback'], ['triggered', 'reason_code', 'action'], 'fallback');
        }
    }

    /**
     * @param  array<string, mixed>  $payload
     * @param  string[]  $allowedKeys
     */
    private static function assertAllowedKeys(array $payload, array $allowedKeys, string $path): void
    {
        $unknownKeys = array_diff(array_keys($payload), $allowedKeys);

        if ($unknownKeys === []) {
            return;
        }

        throw new MediaGenerationContractException(
            'Content draft payload contains unsupported fields.',
            MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
            [
                'path' => $path,
                'unknown_fields' => array_values($unknownKeys),
            ]
        );
    }

    private static function topLevelKeys(): array
    {
        return [
            'schema_version',
            'title',
            'summary',
            'learning_objectives',
            'sections',
            'teacher_delivery_summary',
            'fallback',
            'content_integrity',
        ];
    }

    /**
     * @param  array<string, mixed>  $section
     * @return array<int, array<string, string>>
     */
    private static function fallbackBodyBlocks(array $section, string $resolvedOutputType, array $interpretation): array
    {
        $language = trim((string) ($interpretation['language'] ?? 'id'));
        $usesIndonesian = str_starts_with(strtolower($language), 'id');
        $bullets = array_values(array_filter(
            is_array($section['bullets'] ?? null) ? $section['bullets'] : [],
            static fn (mixed $bullet): bool => is_string($bullet) && trim($bullet) !== ''
        ));
        $title = trim((string) ($section['title'] ?? ''));
        $topicLabel = self::fallbackTopicLabel($interpretation, $usesIndonesian);
        $audienceLabel = trim((string) data_get($interpretation, 'target_audience.label', ''));

        $content = self::fallbackParagraphContent(
            title: $title,
            bullets: $bullets,
            topicLabel: $topicLabel,
            audienceLabel: $audienceLabel,
            resolvedOutputType: $resolvedOutputType,
            usesIndonesian: $usesIndonesian,
        );

        $blocks = [
            [
                'type' => 'paragraph',
                'content' => $content,
            ],
        ];

        $supportingParagraph = self::fallbackSupportingParagraph(
            title: $title,
            topicLabel: $topicLabel,
            audienceLabel: $audienceLabel,
            resolvedOutputType: $resolvedOutputType,
            usesIndonesian: $usesIndonesian,
        );

        if ($supportingParagraph !== null) {
            $blocks[] = [
                'type' => 'paragraph',
                'content' => $supportingParagraph,
            ];
        }

        foreach ($bullets as $bullet) {
            $blocks[] = [
                'type' => self::isPracticeSection($title, $usesIndonesian) ? 'checklist' : 'bullet',
                'content' => trim($bullet),
            ];
        }

        return $blocks;
    }

    /**
     * @param  array<string, mixed>  $interpretation
     */
    private static function fallbackTopicLabel(array $interpretation, bool $usesIndonesian): string
    {
        $subSubject = trim((string) data_get($interpretation, 'sub_subject_context.sub_subject_name', ''));

        if ($subSubject !== '') {
            return $subSubject;
        }

        $subject = trim((string) data_get($interpretation, 'subject_context.subject_name', ''));

        if ($subject !== '') {
            return $subject;
        }

        $title = trim((string) data_get($interpretation, 'document_blueprint.title', ''));

        if ($title !== '') {
            return $title;
        }

        return $usesIndonesian ? 'materi ini' : 'this lesson topic';
    }

    /**
     * @param  string[]  $bullets
     */
    private static function fallbackParagraphContent(
        string $title,
        array $bullets,
        string $topicLabel,
        string $audienceLabel,
        string $resolvedOutputType,
        bool $usesIndonesian,
    ): string {
        $sectionKind = self::fallbackSectionKind($title, $usesIndonesian);
        $focusNarrative = self::fallbackFocusNarrative($bullets, $usesIndonesian);

        if ($usesIndonesian) {
            $audienceSentence = $audienceLabel !== ''
                ? 'Untuk ' . self::lowercaseFirst($audienceLabel) . ', '
                : '';

            $body = match ($sectionKind) {
                'practice' => 'Latihan pada materi ' . $topicLabel . ' membantu siswa menerapkan konsep melalui langkah yang bertahap dan jelas. '
                    . $audienceSentence
                    . ($focusNarrative !== null
                        ? $focusNarrative . '. '
                        : 'Siswa mulai dari contoh yang sederhana lalu beranjak ke latihan yang lebih mandiri. '),
                'intro' => 'Materi ' . $topicLabel . ' membuka pembelajaran dengan ide utama yang perlu dipahami sebelum siswa masuk ke contoh dan latihan. '
                    . $audienceSentence
                    . ($focusNarrative !== null
                        ? $focusNarrative . '. '
                        : 'Bagian ini menyiapkan dasar pemahaman agar pembahasan berikutnya terasa lebih mudah diikuti. '),
                default => 'Pada bagian ini, ' . $topicLabel . ' dijelaskan secara runtut agar siswa melihat hubungan antara konsep, contoh, dan penerapannya. '
                    . $audienceSentence
                    . ($focusNarrative !== null
                        ? $focusNarrative . '. '
                        : 'Pembahasan bergerak dari gagasan utama menuju contoh yang lebih konkret. '),
            };

            return trim(preg_replace('/\s+/u', ' ', $body) ?? $body);
        }

        $audienceSentence = $audienceLabel !== ''
            ? 'For ' . self::lowercaseFirst($audienceLabel) . ', '
            : '';

        $body = match ($sectionKind) {
            'practice' => 'Practice in ' . $topicLabel . ' helps learners apply the concept through clear, step-by-step work. '
                . $audienceSentence
                . ($focusNarrative !== null
                    ? $focusNarrative . '. '
                    : 'Students start with a simple example and continue into more independent work. '),
            'intro' => 'The lesson on ' . $topicLabel . ' opens with the main idea students need before moving into examples and practice. '
                . $audienceSentence
                . ($focusNarrative !== null
                    ? $focusNarrative . '. '
                    : 'This section builds the foundation so the rest of the material is easier to follow. '),
            default => 'In this section, ' . $topicLabel . ' is explained in sequence so students can connect the concept, an example, and its use. '
                . $audienceSentence
                . ($focusNarrative !== null
                    ? $focusNarrative . '. '
                    : 'The explanation moves from the key idea toward a more concrete illustration. '),
        };

        return trim(preg_replace('/\s+/u', ' ', $body) ?? $body);
    }

    private static function fallbackSupportingParagraph(
        string $title,
        string $topicLabel,
        string $audienceLabel,
        string $resolvedOutputType,
        bool $usesIndonesian,
    ): ?string {
        if ($resolvedOutputType === 'pptx') {
            return null;
        }

        $sectionKind = self::fallbackSectionKind($title, $usesIndonesian);
        $audienceLead = $audienceLabel !== ''
            ? ($usesIndonesian
                ? 'Bagi ' . self::lowercaseFirst($audienceLabel) . ', '
                : 'For ' . self::lowercaseFirst($audienceLabel) . ', ')
            : '';

        if ($usesIndonesian) {
            return match ($sectionKind) {
                'practice' => $audienceLead . 'contoh yang bertahap membantu mereka menuliskan langkah, membandingkan jawaban, dan memeriksa alasan di balik setiap hasil pada topik ' . $topicLabel . '.',
                'intro' => $audienceLead . 'contoh yang dekat dengan pengalaman belajar sehari-hari membuat ide utama pada ' . $topicLabel . ' lebih mudah dipahami dan diingat.',
                default => $audienceLead . 'contoh yang konkret dan latihan singkat membantu mereka melihat bagaimana ' . $topicLabel . ' digunakan dalam pembahasan yang lebih nyata.',
            };
        }

        return match ($sectionKind) {
            'practice' => $audienceLead . 'step-by-step examples help learners write their process, compare answers, and check the reasoning behind each result in ' . $topicLabel . '.',
            'intro' => $audienceLead . 'examples that connect to familiar learning situations make the main idea in ' . $topicLabel . ' easier to understand and remember.',
            default => $audienceLead . 'concrete examples and short practice help learners see how ' . $topicLabel . ' appears in more realistic situations.',
        };
    }

    /**
     * @param  string[]  $bullets
     */
    private static function fallbackFocusNarrative(array $bullets, bool $usesIndonesian): ?string
    {
        $items = array_values(array_filter(array_map(
            static function (string $bullet): string {
                $trimmed = trim(preg_replace('/[\s.]+$/u', '', $bullet) ?? $bullet);

                return $trimmed;
            },
            array_slice($bullets, 0, 3)
        )));

        if ($items === []) {
            return null;
        }

        $normalizedItems = array_map(static fn (string $item): string => self::lowercaseFirst($item), $items);
        $joined = self::joinPhrases($normalizedItems, $usesIndonesian ? 'dan' : 'and');

        return $usesIndonesian
            ? 'Hal penting yang ditekankan adalah ' . $joined
            : 'Key points in this section are ' . $joined;
    }

    private static function fallbackSectionKind(string $title, bool $usesIndonesian): string
    {
        $normalizedTitle = strtolower(trim($title));

        $practiceKeywords = $usesIndonesian
            ? ['latihan', 'contoh', 'aktivitas', 'kuis', 'refleksi']
            : ['practice', 'example', 'activity', 'quiz', 'reflection', 'exercise'];

        foreach ($practiceKeywords as $keyword) {
            if (str_contains($normalizedTitle, $keyword)) {
                return 'practice';
            }
        }

        $introKeywords = $usesIndonesian
            ? ['tujuan', 'pengantar', 'pendahuluan', 'konsep dasar']
            : ['goal', 'objective', 'introduction', 'overview', 'foundation'];

        foreach ($introKeywords as $keyword) {
            if (str_contains($normalizedTitle, $keyword)) {
                return 'intro';
            }
        }

        return 'explanation';
    }

    private static function isPracticeSection(string $title, bool $usesIndonesian): bool
    {
        $normalizedTitle = strtolower(trim($title));

        if ($usesIndonesian) {
            return str_contains($normalizedTitle, 'latihan') || str_contains($normalizedTitle, 'refleksi');
        }

        return str_contains($normalizedTitle, 'practice') || str_contains($normalizedTitle, 'reflection');
    }

    /**
     * @param  string[]  $items
     */
    private static function joinPhrases(array $items, string $conjunction): string
    {
        $count = count($items);

        if ($count === 0) {
            return '';
        }

        if ($count === 1) {
            return $items[0];
        }

        if ($count === 2) {
            return $items[0] . ' ' . $conjunction . ' ' . $items[1];
        }

        $last = array_pop($items);

        return implode(', ', $items) . ', ' . $conjunction . ' ' . $last;
    }

    private static function lowercaseFirst(string $value): string
    {
        $trimmed = trim($value);

        if ($trimmed === '') {
            return $trimmed;
        }

        $firstCharacter = mb_substr($trimmed, 0, 1);
        $remaining = mb_substr($trimmed, 1);

        return mb_strtolower($firstCharacter) . $remaining;
    }
}