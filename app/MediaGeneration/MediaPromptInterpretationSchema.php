<?php

namespace App\MediaGeneration;

use Illuminate\Support\Str;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;
use JsonException;

final class MediaPromptInterpretationSchema
{
    public const VERSION = 'media_prompt_understanding.v1';

    public static function allowedOutputFormats(): array
    {
        return ['docx', 'pdf', 'pptx'];
    }

    public static function allowedPreferredOutputTypes(): array
    {
        return [
            'auto',
            ...self::allowedOutputFormats(),
        ];
    }

    public static function llmInstruction(?array $taxonomyHint = null): string
    {
        $instructionLines = [
            'Interpret the teacher request for media generation.',
            'Return exactly one JSON object.',
            'Do not wrap the JSON in markdown fences.',
            'Do not add prose before or after the JSON.',
            'Use schema_version "' . self::VERSION . '".',
            'Always include these top-level keys: schema_version, teacher_prompt, language, teacher_intent, learning_objectives, constraints, output_type_candidates, resolved_output_type_reasoning, document_blueprint, subject_context, sub_subject_context, target_audience, requested_media_characteristics, assets, assessment_or_activity_blocks, teacher_delivery_summary, confidence, fallback.',
            'Use null for unavailable objects and [] for unavailable lists.',
            'Allowed output format values are only: docx, pdf, pptx.',
            'Default to semi-formal Bahasa Indonesia unless the teacher prompt clearly asks for another language.',
            'The text in teacher_intent.goal, learning_objectives, document_blueprint, assessment_or_activity_blocks, and teacher_delivery_summary is not internal planning text. It is the authoring blueprint for the final file and must describe the lesson material that teacher and students are meant to read.',
            'Design the document_blueprint for classroom-ready learning content. When relevant, include sections for topic introduction, concept explanation, formulas or rules, worked example, and short practice or reflection.',
            'When internal taxonomy guidance is available, use it only as a curriculum-alignment hint for subject, grade band, topic scope, and content structure while still honoring the teacher prompt.',
            'Never mention prompts, schema keys, JSON instructions, body_blocks, fallback flags, LLMs, adapters, renderers, or internal workflows in any teacher-facing field.',
        ];

        if (is_array($taxonomyHint)) {
            $instructionLines = array_merge($instructionLines, self::taxonomyInstructionLines($taxonomyHint));
        }

        return implode("\n", $instructionLines);
    }

    public static function decodeAndValidate(string $rawJson): array
    {
        $trimmed = trim($rawJson);

        if ($trimmed === '') {
            throw new MediaGenerationContractException(
                'Prompt interpretation response must not be empty.',
                'llm_contract_failed'
            );
        }

        try {
            $decoded = json_decode($trimmed, true, 512, JSON_THROW_ON_ERROR);
        } catch (JsonException $exception) {
            throw new MediaGenerationContractException(
                'Prompt interpretation returned invalid JSON.',
                'llm_contract_failed',
                ['json_error' => $exception->getMessage()]
            );
        }

        if (! is_array($decoded) || array_is_list($decoded)) {
            throw new MediaGenerationContractException(
                'Prompt interpretation must be a JSON object.',
                'llm_contract_failed'
            );
        }

        return self::validate($decoded);
    }

    public static function validate(array $payload): array
    {
        self::assertAllowedKeys($payload, self::topLevelKeys(), 'payload');
        self::assertNestedAllowedKeys($payload);

        $payload = self::applyDefaults($payload);

        $validator = Validator::make($payload, self::rules());

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Prompt interpretation payload failed schema validation.',
                'llm_contract_failed',
                ['errors' => $validator->errors()->toArray()]
            );
        }

        $normalizedPayload = self::normalize($payload);

        MediaGeneratedContentGuard::assertInterpretationPayload($normalizedPayload);

        return $normalizedPayload;
    }

    public static function fallback(
        string $teacherPrompt,
        string $reasonCode = 'llm_contract_failed',
        ?string $preferredOutputType = null,
        ?string $language = null,
        ?array $subjectContext = null,
        ?array $subSubjectContext = null,
        ?array $taxonomyHint = null,
    ): array {
        $resolvedPreferredOutputType = self::normalizePreferredOutputType($preferredOutputType);
        $candidateTypes = $resolvedPreferredOutputType === 'auto'
            ? self::allowedOutputFormats()
            : [$resolvedPreferredOutputType];
        $candidateScore = $resolvedPreferredOutputType === 'auto' ? 0.34 : 0.51;
        $fallbackLanguage = self::resolveFallbackLanguage($language, $teacherPrompt);
        $usesIndonesian = self::usesIndonesian($fallbackLanguage);
        $normalizedSubjectContext = self::normalizeNamedContext($subjectContext, 'subject_name', 'subject_slug');
        $topicLabel = self::resolveTopicLabel($teacherPrompt, $normalizedSubjectContext, $subSubjectContext, $taxonomyHint);
        $normalizedSubSubjectContext = self::normalizeSubSubjectContext($subSubjectContext, $topicLabel, $normalizedSubjectContext, $taxonomyHint);
        $targetAudience = self::resolveTargetAudience($teacherPrompt, $usesIndonesian);
        $title = self::fallbackTitle($topicLabel, $targetAudience, $usesIndonesian);
        $summary = self::fallbackSummary($topicLabel, $usesIndonesian, $taxonomyHint);
        $sections = self::fallbackSections($topicLabel, $usesIndonesian, $taxonomyHint);

        return self::validate([
            'schema_version' => self::VERSION,
            'teacher_prompt' => $teacherPrompt,
            'language' => $fallbackLanguage,
            'teacher_intent' => [
                'type' => 'generate_learning_media',
                'goal' => $usesIndonesian
                    ? 'Susun materi pembelajaran yang siap dibuka langsung oleh guru dan siswa untuk membahas ' . $topicLabel . '.'
                    : 'Create classroom-ready learning material about ' . $topicLabel . ' that can be opened directly by the teacher and students.',
                'preferred_delivery_mode' => 'digital_download',
                'requires_clarification' => false,
            ],
            'learning_objectives' => self::fallbackLearningObjectives($topicLabel, $usesIndonesian, $taxonomyHint),
            'constraints' => [
                'preferred_output_type' => $resolvedPreferredOutputType,
                'must_include' => [],
                'avoid' => [],
                'tone' => null,
                'max_duration_minutes' => null,
            ],
            'output_type_candidates' => array_map(
                static fn (string $type): array => [
                    'type' => $type,
                    'score' => $candidateScore,
                    'reason' => $usesIndonesian
                        ? 'Format fallback dipilih agar materi tetap bisa dibuat dan dibuka langsung.'
                        : 'Fallback format selected so the lesson can still be generated and opened directly.',
                ],
                $candidateTypes
            ),
            'resolved_output_type_reasoning' => $usesIndonesian
                ? 'Blueprint fallback aman dipakai agar materi tetap tersusun sebagai konten belajar yang siap digunakan.'
                : 'A safe fallback blueprint is used so the lesson still becomes direct classroom material.',
            'document_blueprint' => [
                'title' => $title,
                'summary' => $summary,
                'sections' => $sections,
            ],
            'subject_context' => $normalizedSubjectContext,
            'sub_subject_context' => $normalizedSubSubjectContext,
            'target_audience' => $targetAudience,
            'requested_media_characteristics' => [
                'tone' => null,
                'format_preferences' => $resolvedPreferredOutputType === 'auto' ? [] : [$resolvedPreferredOutputType],
                'visual_density' => null,
            ],
            'assets' => [],
            'assessment_or_activity_blocks' => [
                [
                    'title' => $usesIndonesian ? 'Latihan atau Refleksi Singkat' : 'Short Practice or Reflection',
                    'type' => 'activity',
                    'instructions' => $usesIndonesian
                        ? 'Ajak siswa menjawab pertanyaan singkat atau menyelesaikan satu latihan sederhana yang berkaitan dengan ' . $topicLabel . '.'
                        : 'Ask students to answer a short question or solve one simple task related to ' . $topicLabel . '.',
                ],
            ],
            'teacher_delivery_summary' => self::fallbackTeacherDeliverySummary($topicLabel, $usesIndonesian, $taxonomyHint),
            'confidence' => [
                'score' => 0.0,
                'label' => 'low',
                'rationale' => $usesIndonesian
                    ? 'Blueprint fallback aman dipakai karena respons model sebelumnya tidak dapat digunakan secara langsung.'
                    : 'A safe fallback blueprint is used because the previous model response could not be used directly.',
            ],
            'fallback' => [
                'triggered' => true,
                'reason_code' => $reasonCode,
                'action' => 'use_safe_lesson_blueprint',
            ],
        ]);
    }

    private static function rules(): array
    {
        return [
            'schema_version' => ['required', 'string', Rule::in([self::VERSION])],
            'teacher_prompt' => ['required', 'string', 'max:5000'],
            'language' => ['required', 'string', 'max:32'],
            'teacher_intent' => ['required', 'array'],
            'teacher_intent.type' => ['required', 'string', 'max:100'],
            'teacher_intent.goal' => ['required', 'string', 'max:500'],
            'teacher_intent.preferred_delivery_mode' => ['required', 'string', 'max:100'],
            'teacher_intent.requires_clarification' => ['required', 'boolean'],
            'learning_objectives' => ['present', 'array'],
            'learning_objectives.*' => ['string', 'max:300'],
            'constraints' => ['required', 'array'],
            'constraints.preferred_output_type' => ['required', 'string', Rule::in(self::allowedPreferredOutputTypes())],
            'constraints.max_duration_minutes' => ['nullable', 'integer', 'min:1', 'max:1440'],
            'constraints.must_include' => ['present', 'array'],
            'constraints.must_include.*' => ['string', 'max:300'],
            'constraints.avoid' => ['present', 'array'],
            'constraints.avoid.*' => ['string', 'max:300'],
            'constraints.tone' => ['nullable', 'string', 'max:100'],
            'output_type_candidates' => ['required', 'array', 'min:1'],
            'output_type_candidates.*.type' => ['required', 'string', Rule::in(self::allowedOutputFormats())],
            'output_type_candidates.*.score' => ['required', 'numeric', 'min:0', 'max:1'],
            'output_type_candidates.*.reason' => ['required', 'string', 'max:500'],
            'resolved_output_type_reasoning' => ['required', 'string', 'max:1000'],
            'document_blueprint' => ['required', 'array'],
            'document_blueprint.title' => ['required', 'string', 'max:200'],
            'document_blueprint.summary' => ['required', 'string', 'max:1000'],
            'document_blueprint.sections' => ['required', 'array', 'min:1'],
            'document_blueprint.sections.*.title' => ['required', 'string', 'max:200'],
            'document_blueprint.sections.*.purpose' => ['required', 'string', 'max:500'],
            'document_blueprint.sections.*.bullets' => ['present', 'array'],
            'document_blueprint.sections.*.bullets.*' => ['string', 'max:300'],
            'document_blueprint.sections.*.estimated_length' => ['required', 'string', Rule::in(['short', 'medium', 'long'])],
            'subject_context' => ['nullable', 'array'],
            'subject_context.subject_name' => ['required_with:subject_context', 'string', 'max:100'],
            'subject_context.subject_slug' => ['nullable', 'string', 'max:100'],
            'sub_subject_context' => ['nullable', 'array'],
            'sub_subject_context.sub_subject_name' => ['required_with:sub_subject_context', 'string', 'max:100'],
            'sub_subject_context.sub_subject_slug' => ['nullable', 'string', 'max:100'],
            'target_audience' => ['nullable', 'array'],
            'target_audience.label' => ['required_with:target_audience', 'string', 'max:100'],
            'target_audience.level' => ['nullable', 'string', 'max:100'],
            'target_audience.age_range' => ['nullable', 'string', 'max:100'],
            'requested_media_characteristics' => ['required', 'array'],
            'requested_media_characteristics.tone' => ['nullable', 'string', 'max:100'],
            'requested_media_characteristics.format_preferences' => ['present', 'array'],
            'requested_media_characteristics.format_preferences.*' => ['string', 'max:100'],
            'requested_media_characteristics.visual_density' => ['nullable', 'string', Rule::in(['low', 'medium', 'high'])],
            'assets' => ['present', 'array'],
            'assets.*.type' => ['required', 'string', Rule::in(['text', 'image', 'table', 'chart', 'diagram', 'reference'])],
            'assets.*.description' => ['required', 'string', 'max:500'],
            'assets.*.required' => ['required', 'boolean'],
            'assessment_or_activity_blocks' => ['present', 'array'],
            'assessment_or_activity_blocks.*.title' => ['required', 'string', 'max:200'],
            'assessment_or_activity_blocks.*.type' => ['required', 'string', Rule::in(['assessment', 'activity', 'reflection', 'quiz', 'assignment'])],
            'assessment_or_activity_blocks.*.instructions' => ['required', 'string', 'max:1000'],
            'teacher_delivery_summary' => ['required', 'string', 'max:1000'],
            'confidence' => ['required', 'array'],
            'confidence.score' => ['required', 'numeric', 'min:0', 'max:1'],
            'confidence.label' => ['required', 'string', Rule::in(['low', 'medium', 'high'])],
            'confidence.rationale' => ['nullable', 'string', 'max:500'],
            'fallback' => ['required', 'array'],
            'fallback.triggered' => ['required', 'boolean'],
            'fallback.reason_code' => ['nullable', 'string', 'max:100'],
            'fallback.action' => ['nullable', 'string', 'max:100'],
        ];
    }

    private static function normalize(array $payload): array
    {
        return [
            'schema_version' => self::VERSION,
            'teacher_prompt' => trim($payload['teacher_prompt']),
            'language' => trim($payload['language']),
            'teacher_intent' => [
                'type' => trim($payload['teacher_intent']['type']),
                'goal' => trim($payload['teacher_intent']['goal']),
                'preferred_delivery_mode' => trim($payload['teacher_intent']['preferred_delivery_mode']),
                'requires_clarification' => (bool) $payload['teacher_intent']['requires_clarification'],
            ],
            'learning_objectives' => array_values($payload['learning_objectives']),
            'constraints' => [
                'preferred_output_type' => trim($payload['constraints']['preferred_output_type']),
                'max_duration_minutes' => $payload['constraints']['max_duration_minutes'],
                'must_include' => array_values($payload['constraints']['must_include']),
                'avoid' => array_values($payload['constraints']['avoid']),
                'tone' => $payload['constraints']['tone'] !== null ? trim($payload['constraints']['tone']) : null,
            ],
            'output_type_candidates' => self::normalizeCandidates($payload['output_type_candidates']),
            'resolved_output_type_reasoning' => trim($payload['resolved_output_type_reasoning']),
            'document_blueprint' => [
                'title' => trim($payload['document_blueprint']['title']),
                'summary' => trim($payload['document_blueprint']['summary']),
                'sections' => array_map(
                    static fn (array $section): array => [
                        'title' => trim($section['title']),
                        'purpose' => trim($section['purpose']),
                        'bullets' => array_values($section['bullets']),
                        'estimated_length' => trim($section['estimated_length']),
                    ],
                    $payload['document_blueprint']['sections']
                ),
            ],
            'subject_context' => self::normalizeNullableObject($payload['subject_context']),
            'sub_subject_context' => self::normalizeNullableObject($payload['sub_subject_context']),
            'target_audience' => self::normalizeNullableObject($payload['target_audience']),
            'requested_media_characteristics' => [
                'tone' => $payload['requested_media_characteristics']['tone'] !== null
                    ? trim($payload['requested_media_characteristics']['tone'])
                    : null,
                'format_preferences' => array_values($payload['requested_media_characteristics']['format_preferences']),
                'visual_density' => $payload['requested_media_characteristics']['visual_density'],
            ],
            'assets' => array_map(
                static fn (array $asset): array => [
                    'type' => trim($asset['type']),
                    'description' => trim($asset['description']),
                    'required' => (bool) $asset['required'],
                ],
                $payload['assets']
            ),
            'assessment_or_activity_blocks' => array_map(
                static fn (array $block): array => [
                    'title' => trim($block['title']),
                    'type' => trim($block['type']),
                    'instructions' => trim($block['instructions']),
                ],
                $payload['assessment_or_activity_blocks']
            ),
            'teacher_delivery_summary' => trim($payload['teacher_delivery_summary']),
            'confidence' => [
                'score' => (float) $payload['confidence']['score'],
                'label' => trim($payload['confidence']['label']),
                'rationale' => $payload['confidence']['rationale'] !== null ? trim($payload['confidence']['rationale']) : null,
            ],
            'fallback' => [
                'triggered' => (bool) $payload['fallback']['triggered'],
                'reason_code' => $payload['fallback']['reason_code'] !== null ? trim($payload['fallback']['reason_code']) : null,
                'action' => $payload['fallback']['action'] !== null ? trim($payload['fallback']['action']) : null,
            ],
        ];
    }

    private static function normalizeCandidates(array $candidates): array
    {
        $indexedCandidates = array_map(
            static fn (array $candidate, int $index): array => [
                'index' => $index,
                'type' => trim($candidate['type']),
                'score' => round((float) $candidate['score'], 4),
                'reason' => trim($candidate['reason']),
            ],
            $candidates,
            array_keys($candidates)
        );

        usort($indexedCandidates, static function (array $left, array $right): int {
            if ($left['score'] !== $right['score']) {
                return $right['score'] <=> $left['score'];
            }

            return $left['index'] <=> $right['index'];
        });

        return array_map(static function (array $candidate): array {
            unset($candidate['index']);

            return $candidate;
        }, $indexedCandidates);
    }

    private static function applyDefaults(array $payload): array
    {
        if (! array_key_exists('requested_media_characteristics', $payload)) {
            $payload['requested_media_characteristics'] = [
                'tone' => null,
                'format_preferences' => [],
                'visual_density' => null,
            ];
        } elseif (is_array($payload['requested_media_characteristics'])) {
            $payload['requested_media_characteristics'] = array_merge([
                'tone' => null,
                'format_preferences' => [],
                'visual_density' => null,
            ], $payload['requested_media_characteristics']);
        }

        if (! array_key_exists('constraints', $payload)) {
            return array_merge([
                'subject_context' => null,
                'sub_subject_context' => null,
                'target_audience' => null,
                'assets' => [],
                'assessment_or_activity_blocks' => [],
                'fallback' => [
                    'triggered' => false,
                    'reason_code' => null,
                    'action' => null,
                ],
            ], $payload);
        }

        if (is_array($payload['constraints'])) {
            $payload['constraints'] = array_merge([
                'preferred_output_type' => 'auto',
                'max_duration_minutes' => null,
                'must_include' => [],
                'avoid' => [],
                'tone' => null,
            ], $payload['constraints']);
        }

        if (! array_key_exists('subject_context', $payload)) {
            $payload['subject_context'] = null;
        }

        if (! array_key_exists('sub_subject_context', $payload)) {
            $payload['sub_subject_context'] = null;
        }

        if (! array_key_exists('target_audience', $payload)) {
            $payload['target_audience'] = null;
        }

        if (! array_key_exists('assets', $payload)) {
            $payload['assets'] = [];
        }

        if (! array_key_exists('assessment_or_activity_blocks', $payload)) {
            $payload['assessment_or_activity_blocks'] = [];
        }

        if (! array_key_exists('fallback', $payload)) {
            $payload['fallback'] = [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ];
        } elseif (is_array($payload['fallback'])) {
            $payload['fallback'] = array_merge([
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ], $payload['fallback']);
        }

        return $payload;
    }

    private static function assertNestedAllowedKeys(array $payload): void
    {
        if (isset($payload['teacher_intent']) && is_array($payload['teacher_intent'])) {
            self::assertAllowedKeys($payload['teacher_intent'], ['type', 'goal', 'preferred_delivery_mode', 'requires_clarification'], 'teacher_intent');
        }

        if (isset($payload['constraints']) && is_array($payload['constraints'])) {
            self::assertAllowedKeys($payload['constraints'], ['preferred_output_type', 'max_duration_minutes', 'must_include', 'avoid', 'tone'], 'constraints');
        }

        if (isset($payload['document_blueprint']) && is_array($payload['document_blueprint'])) {
            self::assertAllowedKeys($payload['document_blueprint'], ['title', 'summary', 'sections'], 'document_blueprint');

            if (isset($payload['document_blueprint']['sections']) && is_array($payload['document_blueprint']['sections'])) {
                foreach ($payload['document_blueprint']['sections'] as $index => $section) {
                    if (is_array($section)) {
                        self::assertAllowedKeys($section, ['title', 'purpose', 'bullets', 'estimated_length'], 'document_blueprint.sections.' . $index);
                    }
                }
            }
        }

        if (isset($payload['subject_context']) && is_array($payload['subject_context'])) {
            self::assertAllowedKeys($payload['subject_context'], ['subject_name', 'subject_slug'], 'subject_context');
        }

        if (isset($payload['sub_subject_context']) && is_array($payload['sub_subject_context'])) {
            self::assertAllowedKeys($payload['sub_subject_context'], ['sub_subject_name', 'sub_subject_slug'], 'sub_subject_context');
        }

        if (isset($payload['target_audience']) && is_array($payload['target_audience'])) {
            self::assertAllowedKeys($payload['target_audience'], ['label', 'level', 'age_range'], 'target_audience');
        }

        if (isset($payload['requested_media_characteristics']) && is_array($payload['requested_media_characteristics'])) {
            self::assertAllowedKeys($payload['requested_media_characteristics'], ['tone', 'format_preferences', 'visual_density'], 'requested_media_characteristics');
        }

        if (isset($payload['output_type_candidates']) && is_array($payload['output_type_candidates'])) {
            foreach ($payload['output_type_candidates'] as $index => $candidate) {
                if (is_array($candidate)) {
                    self::assertAllowedKeys($candidate, ['type', 'score', 'reason'], 'output_type_candidates.' . $index);
                }
            }
        }

        if (isset($payload['assets']) && is_array($payload['assets'])) {
            foreach ($payload['assets'] as $index => $asset) {
                if (is_array($asset)) {
                    self::assertAllowedKeys($asset, ['type', 'description', 'required'], 'assets.' . $index);
                }
            }
        }

        if (isset($payload['assessment_or_activity_blocks']) && is_array($payload['assessment_or_activity_blocks'])) {
            foreach ($payload['assessment_or_activity_blocks'] as $index => $block) {
                if (is_array($block)) {
                    self::assertAllowedKeys($block, ['title', 'type', 'instructions'], 'assessment_or_activity_blocks.' . $index);
                }
            }
        }

        if (isset($payload['confidence']) && is_array($payload['confidence'])) {
            self::assertAllowedKeys($payload['confidence'], ['score', 'label', 'rationale'], 'confidence');
        }

        if (isset($payload['fallback']) && is_array($payload['fallback'])) {
            self::assertAllowedKeys($payload['fallback'], ['triggered', 'reason_code', 'action'], 'fallback');
        }
    }

    private static function assertAllowedKeys(array $payload, array $allowedKeys, string $path): void
    {
        $unknownKeys = array_diff(array_keys($payload), $allowedKeys);

        if ($unknownKeys === []) {
            return;
        }

        throw new MediaGenerationContractException(
            'Prompt interpretation payload contains unsupported fields.',
            'llm_contract_failed',
            [
                'path' => $path,
                'unknown_fields' => array_values($unknownKeys),
            ]
        );
    }

    private static function normalizePreferredOutputType(?string $preferredOutputType): string
    {
        if ($preferredOutputType === null || trim($preferredOutputType) === '') {
            return 'auto';
        }

        $normalized = strtolower(trim($preferredOutputType));

        if (! in_array($normalized, self::allowedPreferredOutputTypes(), true)) {
            throw new MediaGenerationContractException(
                'Unsupported preferred output type.',
                'llm_contract_failed',
                ['preferred_output_type' => $preferredOutputType]
            );
        }

        return $normalized;
    }

    private static function normalizeNullableObject(mixed $value): ?array
    {
        return is_array($value) ? $value : null;
    }

    private static function resolveFallbackLanguage(?string $language, string $teacherPrompt): string
    {
        if (is_string($language) && trim($language) !== '' && trim($language) !== 'und') {
            return trim($language);
        }

        return self::looksLikeIndonesianPrompt($teacherPrompt) ? 'id' : 'en';
    }

    private static function usesIndonesian(string $language): bool
    {
        return str_starts_with(strtolower(trim($language)), 'id');
    }

    /**
     * @param  array<string, mixed>|null  $subjectContext
     * @return array<string, string|null>|null
     */
    private static function normalizeNamedContext(?array $subjectContext, string $nameKey, string $slugKey): ?array
    {
        if (! is_array($subjectContext)) {
            return null;
        }

        $name = trim((string) ($subjectContext[$nameKey] ?? ''));

        if ($name === '') {
            return null;
        }

        $slug = trim((string) ($subjectContext[$slugKey] ?? ''));

        return [
            $nameKey => $name,
            $slugKey => $slug !== '' ? $slug : Str::slug($name),
        ];
    }

    /**
     * @param  array<string, mixed>|null  $subjectContext
     * @param  array<string, mixed>|null  $subSubjectContext
     */
    private static function resolveTopicLabel(string $teacherPrompt, ?array $subjectContext, ?array $subSubjectContext, ?array $taxonomyHint = null): string
    {
        $subSubjectName = trim((string) data_get($subSubjectContext, 'sub_subject_name', ''));

        if ($subSubjectName !== '') {
            return $subSubjectName;
        }

        $extractedTopic = self::extractTopicFromPrompt($teacherPrompt);

        if ($extractedTopic !== null) {
            return $extractedTopic;
        }

        $taxonomySubSubject = trim((string) data_get($taxonomyHint, 'best_match.sub_subject_name', ''));

        if ($taxonomySubSubject !== '') {
            return $taxonomySubSubject;
        }

        $subjectName = trim((string) data_get($subjectContext, 'subject_name', ''));

        if ($subjectName !== '') {
            return $subjectName;
        }

        $taxonomySubject = trim((string) data_get($taxonomyHint, 'best_match.subject_name', ''));

        if ($taxonomySubject !== '') {
            return $taxonomySubject;
        }

        return self::looksLikeIndonesianPrompt($teacherPrompt) ? 'materi pembelajaran' : 'the requested lesson topic';
    }

    /**
     * @param  array<string, mixed>|null  $subSubjectContext
     * @param  array<string, mixed>|null  $subjectContext
     * @return array<string, string|null>|null
     */
    private static function normalizeSubSubjectContext(?array $subSubjectContext, string $topicLabel, ?array $subjectContext, ?array $taxonomyHint = null): ?array
    {
        $normalized = self::normalizeNamedContext($subSubjectContext, 'sub_subject_name', 'sub_subject_slug');

        if ($normalized !== null) {
            return $normalized;
        }

        $taxonomySubSubject = trim((string) data_get($taxonomyHint, 'best_match.sub_subject_name', ''));

        if ($taxonomySubSubject !== '') {
            return [
                'sub_subject_name' => $taxonomySubSubject,
                'sub_subject_slug' => trim((string) data_get($taxonomyHint, 'best_match.sub_subject_slug', '')) !== ''
                    ? trim((string) data_get($taxonomyHint, 'best_match.sub_subject_slug'))
                    : Str::slug($taxonomySubSubject),
            ];
        }

        $subjectName = trim((string) data_get($subjectContext, 'subject_name', ''));

        if ($topicLabel === '' || strcasecmp($topicLabel, $subjectName) === 0) {
            return null;
        }

        return [
            'sub_subject_name' => $topicLabel,
            'sub_subject_slug' => Str::slug($topicLabel),
        ];
    }

    /**
     * @return array<string, string>|null
     */
    private static function resolveTargetAudience(string $teacherPrompt, bool $usesIndonesian): ?array
    {
        $normalizedPrompt = strtolower(trim($teacherPrompt));

        if (preg_match('/\bkelas\s+(\d{1,2})\b/u', $teacherPrompt, $matches) === 1) {
            $grade = trim($matches[1]);

            return [
                'label' => 'Siswa kelas ' . $grade,
                'level' => self::gradeLevelFromNumber((int) $grade),
                'age_range' => self::ageRangeFromNumber((int) $grade),
            ];
        }

        if (preg_match('/\bgrade\s+(\d{1,2})\b/i', $teacherPrompt, $matches) === 1) {
            $grade = trim($matches[1]);

            return [
                'label' => 'Grade ' . $grade . ' students',
                'level' => self::gradeLevelFromNumber((int) $grade),
                'age_range' => self::ageRangeFromNumber((int) $grade),
            ];
        }

        if (str_contains($normalizedPrompt, 'sma') || str_contains($normalizedPrompt, 'highschool') || str_contains($normalizedPrompt, 'high school')) {
            return [
                'label' => $usesIndonesian ? 'Siswa SMA' : 'High school students',
                'level' => 'high_school',
                'age_range' => '15-18',
            ];
        }

        if (str_contains($normalizedPrompt, 'smp') || str_contains($normalizedPrompt, 'middle school')) {
            return [
                'label' => $usesIndonesian ? 'Siswa SMP' : 'Middle school students',
                'level' => 'middle_school',
                'age_range' => '12-15',
            ];
        }

        if (str_contains($normalizedPrompt, 'sd') || str_contains($normalizedPrompt, 'elementary')) {
            return [
                'label' => $usesIndonesian ? 'Siswa sekolah dasar' : 'Elementary students',
                'level' => 'elementary',
                'age_range' => '7-12',
            ];
        }

        return null;
    }

    private static function gradeLevelFromNumber(int $grade): string
    {
        return match (true) {
            $grade <= 6 => 'elementary',
            $grade <= 9 => 'middle_school',
            default => 'high_school',
        };
    }

    private static function ageRangeFromNumber(int $grade): string
    {
        return match (true) {
            $grade <= 1 => '6-7',
            $grade <= 3 => '7-9',
            $grade <= 6 => '9-12',
            $grade <= 9 => '12-15',
            default => '15-18',
        };
    }

    private static function fallbackTitle(string $topicLabel, ?array $targetAudience, bool $usesIndonesian): string
    {
        $audienceLabel = trim((string) data_get($targetAudience, 'label', ''));

        if ($usesIndonesian) {
            return $audienceLabel !== ''
                ? 'Materi ' . $topicLabel . ' untuk ' . $audienceLabel
                : 'Materi Pembelajaran ' . $topicLabel;
        }

        return $audienceLabel !== ''
            ? $topicLabel . ' Learning Material for ' . $audienceLabel
            : 'Learning Material: ' . $topicLabel;
    }

    private static function fallbackSummary(string $topicLabel, bool $usesIndonesian, ?array $taxonomyHint = null): string
    {
        $description = trim((string) data_get($taxonomyHint, 'best_match.description', ''));

        if ($description !== '') {
            return $usesIndonesian
                ? $description . ' Materi disusun dengan penjelasan bertahap, contoh, dan latihan singkat agar siap dipakai di kelas.'
                : $description . ' The lesson is organized with step-by-step explanation, examples, and short practice so it is ready for classroom use.';
        }

        return $usesIndonesian
            ? 'Materi ini merangkum konsep inti ' . $topicLabel . ', penjelasan pokok, contoh sederhana, dan latihan singkat agar siap dipakai dalam pembelajaran.'
            : 'This material summarizes the core ideas of ' . $topicLabel . ', the main explanation, a simple example, and short practice so it is ready for classroom use.';
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private static function fallbackSections(string $topicLabel, bool $usesIndonesian, ?array $taxonomyHint = null): array
    {
        $structureItems = self::taxonomyStructureItems($taxonomyHint);

        if ($structureItems !== []) {
            $sections = self::fallbackSectionsFromStructure($topicLabel, $usesIndonesian, $structureItems);

            if (count($sections) >= 4) {
                return array_slice($sections, 0, 4);
            }

            foreach (self::defaultFallbackSections($topicLabel, $usesIndonesian) as $defaultSection) {
                if (count($sections) >= 4) {
                    break;
                }

                $sections[] = $defaultSection;
            }

            return $sections;
        }

        return self::defaultFallbackSections($topicLabel, $usesIndonesian);
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private static function defaultFallbackSections(string $topicLabel, bool $usesIndonesian): array
    {
        if ($usesIndonesian) {
            return [
                [
                    'title' => 'Pengantar ' . $topicLabel,
                    'purpose' => 'Membuka pembelajaran dengan gambaran umum, tujuan belajar, dan alasan pentingnya topik ini.',
                    'bullets' => [
                        'Pengertian awal tentang ' . $topicLabel,
                        'Istilah utama yang perlu dikenali siswa',
                    ],
                    'estimated_length' => 'short',
                ],
                [
                    'title' => 'Konsep dan Aturan Utama',
                    'purpose' => 'Menjelaskan ide inti, aturan, rumus, atau langkah yang relevan dengan topik ini.',
                    'bullets' => [
                        'Konsep inti yang harus dipahami',
                        'Aturan, rumus, atau langkah kerja yang relevan bila ada',
                    ],
                    'estimated_length' => 'medium',
                ],
                [
                    'title' => 'Contoh Penerapan',
                    'purpose' => 'Menunjukkan satu contoh yang dibahas langkah demi langkah agar siswa melihat penerapan konsep.',
                    'bullets' => [
                        'Satu contoh sederhana yang dekat dengan materi',
                        'Penjelasan alasan di setiap langkah',
                    ],
                    'estimated_length' => 'medium',
                ],
                [
                    'title' => 'Latihan Singkat dan Refleksi',
                    'purpose' => 'Memberikan kesempatan kepada siswa untuk mencoba dan meninjau kembali ide utama.',
                    'bullets' => [
                        'Pertanyaan atau latihan singkat untuk menguji pemahaman',
                        'Ajak siswa merangkum kembali ide utama dengan bahasa sendiri',
                    ],
                    'estimated_length' => 'short',
                ],
            ];
        }

        return [
            [
                'title' => 'Introduction to ' . $topicLabel,
                'purpose' => 'Open the lesson with a clear overview, learning goals, and the importance of the topic.',
                'bullets' => [
                    'A simple starting explanation of ' . $topicLabel,
                    'Key terms students need to recognize',
                ],
                'estimated_length' => 'short',
            ],
            [
                'title' => 'Core Concepts and Rules',
                'purpose' => 'Explain the main ideas, rules, formulas, or steps that matter for the topic.',
                'bullets' => [
                    'The central idea students need to understand',
                    'Rules, formulas, or steps that apply when relevant',
                ],
                'estimated_length' => 'medium',
            ],
            [
                'title' => 'Worked Example',
                'purpose' => 'Show a simple example step by step so students can see the concept in use.',
                'bullets' => [
                    'A simple example connected to the lesson',
                    'A short explanation for each step',
                ],
                'estimated_length' => 'medium',
            ],
            [
                'title' => 'Short Practice and Reflection',
                'purpose' => 'Give students a quick chance to practice and restate the main idea.',
                'bullets' => [
                    'One or two short tasks to check understanding',
                    'A reflection prompt to restate the main idea',
                ],
                'estimated_length' => 'short',
            ],
        ];
    }

    /**
     * @return string[]
     */
    private static function fallbackLearningObjectives(string $topicLabel, bool $usesIndonesian, ?array $taxonomyHint = null): array
    {
        $structureItems = self::taxonomyStructureItems($taxonomyHint);

        if ($structureItems !== []) {
            $firstStructureItem = $structureItems[0];
            $secondStructureItem = $structureItems[1] ?? null;

            if ($usesIndonesian) {
                return array_values(array_filter([
                    'Siswa memahami konsep utama tentang ' . $topicLabel . ' melalui bagian ' . $firstStructureItem . '.',
                    $secondStructureItem !== null
                        ? 'Siswa dapat menjelaskan aturan, langkah, atau rincian penting pada bagian ' . $secondStructureItem . '.'
                        : null,
                    'Siswa mencoba contoh, latihan, atau refleksi singkat yang berkaitan dengan ' . $topicLabel . '.',
                ]));
            }

            return array_values(array_filter([
                'Students understand the main ideas of ' . $topicLabel . ' through the ' . $firstStructureItem . ' section.',
                $secondStructureItem !== null
                    ? 'Students explain the relevant rules, steps, or important details from the ' . $secondStructureItem . ' section.'
                    : null,
                'Students try a short example, practice task, or reflection connected to ' . $topicLabel . '.',
            ]));
        }

        if ($usesIndonesian) {
            return [
                'Siswa memahami gagasan utama tentang ' . $topicLabel . '.',
                'Siswa dapat menjelaskan istilah, aturan, atau langkah penting yang berkaitan dengan ' . $topicLabel . '.',
                'Siswa mencoba contoh atau latihan singkat yang berhubungan dengan ' . $topicLabel . '.',
            ];
        }

        return [
            'Students understand the main idea of ' . $topicLabel . '.',
            'Students explain the key terms, rules, or steps related to ' . $topicLabel . '.',
            'Students try a short example or practice task connected to ' . $topicLabel . '.',
        ];
    }

    private static function fallbackTeacherDeliverySummary(string $topicLabel, bool $usesIndonesian, ?array $taxonomyHint = null): string
    {
        $structureItems = self::taxonomyStructureItems($taxonomyHint);

        if ($structureItems !== []) {
            $structureFlow = implode(', ', array_slice($structureItems, 0, 4));

            return $usesIndonesian
                ? 'Gunakan materi ini untuk membahas ' . $topicLabel . ' dengan alur ' . $structureFlow . ', lalu tutup dengan latihan atau refleksi singkat.'
                : 'Use this material to teach ' . $topicLabel . ' through the flow ' . $structureFlow . ', then close with short practice or reflection.';
        }

        return $usesIndonesian
            ? 'Gunakan materi ini untuk membuka pembelajaran ' . $topicLabel . ', menegaskan konsep inti, lalu menutup dengan contoh dan latihan singkat.'
            : 'Use this material to introduce ' . $topicLabel . ', reinforce the core concept, and close with an example and short practice.';
    }

    /**
     * @param  array<string, mixed>  $taxonomyHint
     * @return string[]
     */
    private static function taxonomyInstructionLines(array $taxonomyHint): array
    {
        $bestMatch = (array) data_get($taxonomyHint, 'best_match', []);
        $instructionLines = ['Internal taxonomy guidance for alignment only:'];

        foreach ([
            'jenjang' => 'Grade band',
            'kelas' => 'Class',
            'semester' => 'Semester',
            'bab' => 'Chapter',
        ] as $key => $label) {
            $value = data_get($bestMatch, $key);

            if ($value !== null && $value !== '') {
                $instructionLines[] = '- ' . $label . ': ' . $value;
            }
        }

        foreach ([
            'subject_name' => 'Subject',
            'sub_subject_name' => 'Sub-subject',
            'description' => 'Topic description',
            'content_structure' => 'Expected content structure',
        ] as $key => $label) {
            $value = trim((string) data_get($bestMatch, $key, ''));

            if ($value !== '') {
                $instructionLines[] = '- ' . $label . ': ' . $value;
            }
        }

        $confidenceScore = data_get($taxonomyHint, 'confidence.score');
        $confidenceLabel = trim((string) data_get($taxonomyHint, 'confidence.label', ''));

        if ($confidenceScore !== null || $confidenceLabel !== '') {
            $instructionLines[] = '- Confidence: ' . trim(implode(' ', array_filter([
                $confidenceLabel !== '' ? $confidenceLabel : null,
                is_numeric($confidenceScore) ? '(' . number_format((float) $confidenceScore, 2) . ')' : null,
            ])));
        }

        return $instructionLines;
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private static function fallbackSectionsFromStructure(string $topicLabel, bool $usesIndonesian, array $structureItems): array
    {
        $sections = [];
        $structureItems = array_slice($structureItems, 0, 4);
        $lastIndex = count($structureItems) - 1;

        foreach ($structureItems as $index => $structureItem) {
            $sections[] = [
                'title' => self::fallbackSectionTitleFromStructureItem($structureItem, $topicLabel, $usesIndonesian),
                'purpose' => self::fallbackSectionPurposeFromStructureItem($structureItem, $topicLabel, $usesIndonesian),
                'bullets' => self::fallbackSectionBulletsFromStructureItem($structureItem, $topicLabel, $usesIndonesian),
                'estimated_length' => ($index === 0 || $index === $lastIndex) ? 'short' : 'medium',
            ];
        }

        return $sections;
    }

    private static function fallbackSectionTitleFromStructureItem(string $structureItem, string $topicLabel, bool $usesIndonesian): string
    {
        return match (self::fallbackStructureKind($structureItem)) {
            'concept' => $usesIndonesian ? 'Konsep Inti ' . $topicLabel : 'Core Concepts of ' . $topicLabel,
            'rules' => Str::headline($structureItem),
            'example' => Str::headline($structureItem),
            'practice' => Str::headline($structureItem),
            default => Str::headline($structureItem),
        };
    }

    private static function fallbackSectionPurposeFromStructureItem(string $structureItem, string $topicLabel, bool $usesIndonesian): string
    {
        return match (self::fallbackStructureKind($structureItem)) {
            'concept' => $usesIndonesian
                ? 'Menjelaskan definisi, gagasan pokok, dan makna penting dari ' . $topicLabel . ' secara bertahap.'
                : 'Explain the definition, central ideas, and key meaning of ' . $topicLabel . ' step by step.',
            'rules' => $usesIndonesian
                ? 'Merangkum aturan, rumus, prosedur, atau rincian kerja yang perlu diikuti saat mempelajari ' . $topicLabel . '.'
                : 'Summarize the rules, formulas, procedures, or working details students need for ' . $topicLabel . '.',
            'example' => $usesIndonesian
                ? 'Menunjukkan contoh, kasus, atau penerapan nyata agar siswa melihat ' . $topicLabel . ' dalam praktik.'
                : 'Show examples, cases, or practical application so students can see ' . $topicLabel . ' in use.',
            'practice' => $usesIndonesian
                ? 'Memberikan ruang latihan, diskusi, evaluasi, atau refleksi agar pemahaman siswa dapat dicek.'
                : 'Provide practice, discussion, evaluation, or reflection so student understanding can be checked.',
            default => $usesIndonesian
                ? 'Mengembangkan bagian materi yang mendukung pemahaman menyeluruh tentang ' . $topicLabel . '.'
                : 'Develop a lesson section that supports a complete understanding of ' . $topicLabel . '.',
        };
    }

    /**
     * @return string[]
     */
    private static function fallbackSectionBulletsFromStructureItem(string $structureItem, string $topicLabel, bool $usesIndonesian): array
    {
        return match (self::fallbackStructureKind($structureItem)) {
            'concept' => $usesIndonesian
                ? [
                    'Istilah utama yang perlu dikenali siswa pada topik ' . $topicLabel,
                    'Hubungan konsep inti dengan situasi belajar sehari-hari',
                ]
                : [
                    'Key terms students need to recognize in ' . $topicLabel,
                    'How the central idea connects to classroom or everyday situations',
                ],
            'rules' => $usesIndonesian
                ? [
                    'Aturan, rumus, langkah, atau prosedur yang relevan',
                    'Kapan dan bagaimana bagian ' . $structureItem . ' digunakan dalam materi',
                ]
                : [
                    'Relevant rules, formulas, steps, or procedures',
                    'When and how the ' . $structureItem . ' content is used in the lesson',
                ],
            'example' => $usesIndonesian
                ? [
                    'Contoh, kasus, atau fenomena yang dekat dengan ' . $topicLabel,
                    'Penjelasan langkah atau alasan pada tiap tahap penerapan',
                ]
                : [
                    'A concrete example, case, or phenomenon related to ' . $topicLabel,
                    'A short explanation of the steps or reasoning in the application',
                ],
            'practice' => $usesIndonesian
                ? [
                    'Latihan singkat, diskusi, atau evaluasi untuk mengecek pemahaman',
                    'Refleksi atau tindak lanjut sederhana setelah mempelajari ' . $topicLabel,
                ]
                : [
                    'Short practice, discussion, or evaluation to check understanding',
                    'A reflection or simple follow-up after learning ' . $topicLabel,
                ],
            default => $usesIndonesian
                ? [
                    'Poin penting yang mendukung pemahaman ' . $topicLabel,
                    'Transisi yang membantu siswa mengikuti alur materi',
                ]
                : [
                    'Supporting points that strengthen understanding of ' . $topicLabel,
                    'Transitions that help students follow the lesson flow',
                ],
        };
    }

    private static function fallbackStructureKind(string $structureItem): string
    {
        $normalized = self::normalizeStructureLabel($structureItem);

        return match (true) {
            preg_match('/latihan|evaluasi|refleksi|diskusi|penugasan|quiz|kuis/u', $normalized) === 1 => 'practice',
            preg_match('/contoh|fenomena|kasus|aplikasi|resep|bacaan|praktik|servis|troubleshooting/u', $normalized) === 1 => 'example',
            preg_match('/rumus|hukum|teorema|sifat|landasan|prosedur|alat|bahan|keselamatan|k3|diagram|skema|prinsip|komponan|komponen|unsur|pembuktian|proof|metode/u', $normalized) === 1 => 'rules',
            default => 'concept',
        };
    }

    /**
     * @return string[]
     */
    private static function taxonomyStructureItems(?array $taxonomyHint): array
    {
        $structureItems = data_get($taxonomyHint, 'best_match.structure_items', []);

        if (! is_array($structureItems)) {
            return [];
        }

        return array_values(array_filter(array_map(
            static fn (mixed $structureItem): string => Str::of((string) $structureItem)->trim()->toString(),
            $structureItems
        ), static fn (string $structureItem): bool => $structureItem !== ''));
    }

    private static function normalizeStructureLabel(string $value): string
    {
        $normalized = Str::ascii($value);
        $normalized = strtolower($normalized);
        $normalized = preg_replace('/[^\p{L}\p{N}]+/u', ' ', $normalized) ?? $normalized;

        return trim($normalized);
    }

    private static function looksLikeIndonesianPrompt(string $teacherPrompt): bool
    {
        return preg_match('/\b(buatkan|untuk|kelas|siswa|materi|pembelajaran|pelajaran|guru|ajar)\b/iu', $teacherPrompt) === 1;
    }

    private static function extractTopicFromPrompt(string $teacherPrompt): ?string
    {
        $patterns = [
            '/(?:mata\s+pelajaran|pelajaran|materi|topik)\s+([\p{L}\p{N}\s\-]{3,80}?)(?:\s+(?:yang|untuk|dengan|agar|supaya)\b|[\.,!?]|$)/iu',
            '/(?:tentang|mengenai|about)\s+([\p{L}\p{N}\s\-]{3,80}?)(?:\s+(?:yang|untuk|dengan|agar|supaya)\b|[\.,!?]|$)/iu',
        ];

        foreach ($patterns as $pattern) {
            if (preg_match($pattern, $teacherPrompt, $matches) === 1) {
                $candidate = trim($matches[1]);

                if ($candidate !== '') {
                    return Str::of($candidate)
                        ->replaceMatches('/\s+/', ' ')
                        ->trim()
                        ->title()
                        ->toString();
                }
            }
        }

        $stopwords = [
            'buatkan', 'aku', 'saya', 'sebuah', 'suatu', 'pdf', 'docx', 'pptx', 'slide', 'slides', 'deck',
            'materi', 'pembelajaran', 'belajar', 'pelajaran', 'mata', 'topik', 'tentang', 'mengenai',
            'untuk', 'yang', 'dengan', 'agar', 'supaya', 'bisa', 'ku', 'ajarkan', 'ke', 'siswa', 'siswi',
            'siswa-siswa', 'guru', 'handout', 'printable', 'file', 'create', 'make', 'lesson', 'learning',
            'material', 'teaching', 'the', 'a', 'an', 'of', 'for', 'my', 'students', 'student', 'class',
            'kelas', 'highschool', 'high', 'school', 'can', 'be', 'opened', 'langsung', 'siap', 'pakai',
        ];

        $tokens = preg_split('/[^\p{L}\p{N}\-]+/u', strtolower($teacherPrompt)) ?: [];
        $candidates = array_values(array_filter($tokens, static function (string $token) use ($stopwords): bool {
            return $token !== ''
                && ! in_array($token, $stopwords, true)
                && mb_strlen($token) >= 4
                && ! ctype_digit($token);
        }));

        if ($candidates === []) {
            return null;
        }

        return Str::of(implode(' ', array_slice($candidates, 0, 3)))
            ->replaceMatches('/\s+/', ' ')
            ->trim()
            ->title()
            ->toString();
    }

    private static function topLevelKeys(): array
    {
        return [
            'schema_version',
            'teacher_prompt',
            'language',
            'teacher_intent',
            'learning_objectives',
            'constraints',
            'output_type_candidates',
            'resolved_output_type_reasoning',
            'document_blueprint',
            'subject_context',
            'sub_subject_context',
            'target_audience',
            'requested_media_characteristics',
            'assets',
            'assessment_or_activity_blocks',
            'teacher_delivery_summary',
            'confidence',
            'fallback',
        ];
    }
}