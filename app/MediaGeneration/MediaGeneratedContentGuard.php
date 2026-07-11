<?php

namespace App\MediaGeneration;

final class MediaGeneratedContentGuard
{
    /**
     * @var array<string, string>
     */
    private const DRAFT_SCAFFOLD_PATTERNS = [
        '/\bbagian ini disusun untuk\b/iu' => 'outline_scaffold',
        '/\bfokus utamanya meliputi\b/iu' => 'outline_scaffold',
        '/\bjelaskan ide pokoknya secara runtut\b/iu' => 'outline_scaffold',
        '/\bsampaikan inti materinya secara singkat, jelas, dan mudah dipresentasikan\b/iu' => 'outline_scaffold',
        '/\bdorong siswa merangkum kembali inti\b/iu' => 'outline_scaffold',
        '/\bthis section is written for\b/iu' => 'outline_scaffold',
        '/\bthe main focus includes\b/iu' => 'outline_scaffold',
        '/\bpresent the main idea in sequence\b/iu' => 'outline_scaffold',
        '/\bkeep the explanation concise, clear, and ready for presentation\b/iu' => 'outline_scaffold',
        '/\bencourage students to restate the key idea\b/iu' => 'outline_scaffold',
    ];

    /**
     * @var array<string, string>
     */
    private const FORBIDDEN_PATTERNS = [
        '/return exactly one json object/iu' => 'json_contract_instruction',
        '/do not wrap the json/iu' => 'json_contract_instruction',
        '/do not add prose before or after the json/iu' => 'json_contract_instruction',
        '/use schema_version/iu' => 'schema_instruction',
        '/always include these top-level keys/iu' => 'schema_instruction',
        '/top-level keys\s*:/iu' => 'schema_instruction',
        '/each sections entry must include/iu' => 'schema_instruction',
        '/each body_blocks entry/iu' => 'schema_instruction',
        '/body_blocks\.content/iu' => 'schema_instruction',
        '/fallback\.triggered/iu' => 'schema_instruction',
        '/adapter contract guardrails/iu' => 'adapter_instruction',
        '/json[- ]only output/iu' => 'json_contract_instruction',
        '/media_content_draft\.v1/iu' => 'schema_version_leak',
        '/media_prompt_understanding\.v1/iu' => 'schema_version_leak',
        '/re-run interpretation with json-only output/iu' => 'pipeline_instruction',
        '/retry prompt interpretation/iu' => 'pipeline_instruction',
        '/before any media file is rendered/iu' => 'pipeline_instruction',
        '/before sending any artifact request to the renderer/iu' => 'pipeline_instruction',
        '/internal taxonomy guidance for alignment only/iu' => 'taxonomy_instruction',
        '/curriculum-alignment hint/iu' => 'taxonomy_instruction',
    ];

    private const META_INSTRUCTION_PATTERNS = [
        'procedural_instruction' => '/\b(follow these steps|implement this|set up|ensure (teachers?|students?|that|you) have|prepare (the|students|a))\b/iu',
        'conversational_filler' => '/\b(here is your|i have (generated|created|prepared)|i\'ve|as (an ai|a language model|claude|chatgpt)|according to my analysis)\b/iu',
        'structural_scaffolding' => '/\b(this (section|lesson|activity) (is designed to|aims to|will|focuses on)|focus on the following|be sure to|the purpose of this)\b/iu',
    ];

    /**
     * @param  array<string, mixed>  $payload
     */
    public static function assertInterpretationPayload(array $payload): void
    {
        $violations = [];
        $add = function (array $v) use (&$violations) {
            foreach ($v as $violation) {
                $violations[] = $violation;
            }
        };

        $add(self::assertTextSafe('teacher_intent.goal', data_get($payload, 'teacher_intent.goal')));

        foreach (array_values((array) data_get($payload, 'learning_objectives', [])) as $index => $objective) {
            $add(self::assertLearningObjective('learning_objectives.' . $index, $objective));
        }

        foreach (array_values((array) data_get($payload, 'constraints.must_include', [])) as $index => $constraint) {
            $add(self::assertTextSafe('constraints.must_include.' . $index, $constraint));
        }

        foreach (array_values((array) data_get($payload, 'constraints.avoid', [])) as $index => $constraint) {
            $add(self::assertTextSafe('constraints.avoid.' . $index, $constraint));
        }

        foreach (array_values((array) data_get($payload, 'output_type_candidates', [])) as $index => $candidate) {
            $add(self::assertTextSafe('output_type_candidates.' . $index . '.reason', data_get($candidate, 'reason')));
        }

        $add(self::assertTextSafe('resolved_output_type_reasoning', data_get($payload, 'resolved_output_type_reasoning')));
        $add(self::assertTextSafe('document_blueprint.title', data_get($payload, 'document_blueprint.title')));
        $add(self::assertTextSafe('document_blueprint.summary', data_get($payload, 'document_blueprint.summary')));

        foreach (array_values((array) data_get($payload, 'document_blueprint.sections', [])) as $sectionIndex => $section) {
            $add(self::assertTextSafe('document_blueprint.sections.' . $sectionIndex . '.title', data_get($section, 'title')));
            $add(self::assertSectionPurpose('document_blueprint.sections.' . $sectionIndex . '.purpose', data_get($section, 'purpose')));

            foreach (array_values((array) data_get($section, 'bullets', [])) as $bulletIndex => $bullet) {
                $add(self::assertTextSafe('document_blueprint.sections.' . $sectionIndex . '.bullets.' . $bulletIndex, $bullet));
            }
        }

        foreach (array_values((array) data_get($payload, 'assessment_or_activity_blocks', [])) as $index => $block) {
            $add(self::assertTextSafe('assessment_or_activity_blocks.' . $index . '.title', data_get($block, 'title')));
            $add(self::assertAssessmentInstructions('assessment_or_activity_blocks.' . $index . '.instructions', data_get($block, 'instructions')));
        }

        $add(self::assertTeacherDeliverySummary('teacher_delivery_summary', data_get($payload, 'teacher_delivery_summary')));
        $add(self::assertTextSafe('confidence.rationale', data_get($payload, 'confidence.rationale')));

        if ($violations !== []) {
            throw new MediaGenerationContractException(
                'Payload failed integrity checks.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                ['violations' => $violations]
            );
        }
    }

    public static function assertContentDraftPayload(array $payload, ?string $resolvedOutputType = null): void
    {
        $violations = [];
        $add = function (array $v) use (&$violations) {
            foreach ($v as $violation) {
                $violations[] = $violation;
            }
        };

        $add(self::assertTextSafe('title', data_get($payload, 'title')));
        $add(self::assertTextSafe('summary', data_get($payload, 'summary')));
        $add(self::assertTeacherDeliverySummary('teacher_delivery_summary', data_get($payload, 'teacher_delivery_summary')));
        $add(self::assertDraftMaterialText('summary', data_get($payload, 'summary')));

        foreach (array_values((array) data_get($payload, 'learning_objectives', [])) as $index => $objective) {
            $add(self::assertLearningObjective('learning_objectives.' . $index, $objective));
        }

        foreach (array_values((array) data_get($payload, 'sections', [])) as $sectionIndex => $section) {
            $add(self::assertTextSafe('sections.' . $sectionIndex . '.title', data_get($section, 'title')));
            $add(self::assertSectionPurpose('sections.' . $sectionIndex . '.purpose', data_get($section, 'purpose')));

            foreach (array_values((array) data_get($section, 'body_blocks', [])) as $blockIndex => $block) {
                $add(self::assertTextSafe('sections.' . $sectionIndex . '.body_blocks.' . $blockIndex . '.content', data_get($block, 'content')));
                $add(self::assertDraftMaterialText(
                    'sections.' . $sectionIndex . '.body_blocks.' . $blockIndex . '.content',
                    data_get($block, 'content')
                ));
            }
        }

        if ($violations !== []) {
            throw new MediaGenerationContractException(
                'Content draft failed integrity checks.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                ['violations' => $violations]
            );
        }

        self::assertSectionNarrative((array) data_get($payload, 'sections', []), $resolvedOutputType);
    }

    private static function assertSectionNarrative(array $sections, ?string $resolvedOutputType): void
    {
        $normalizedOutputType = strtolower(trim((string) $resolvedOutputType));

        if ($normalizedOutputType === 'pptx') {
            return;
        }

        foreach ($sections as $sectionIndex => $section) {
            $paragraphBlocks = array_values(array_filter(
                (array) ($section['body_blocks'] ?? []),
                static function (mixed $block): bool {
                    if (! is_array($block)) {
                        return false;
                    }

                    return ($block['type'] ?? null) === 'paragraph'
                        && self::textLength((string) ($block['content'] ?? '')) >= 60;
                }
            ));

            if ($paragraphBlocks !== []) {
                continue;
            }

            throw new MediaGenerationContractException(
                'Document content must include at least one explanatory paragraph in every section.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                [
                    'path' => 'sections.' . $sectionIndex . '.body_blocks',
                    'reason' => 'missing_explanatory_paragraph',
                ]
            );
        }
    }

    public static function assertLearningObjective(string $path, mixed $value): array
    {
        if (! is_string($value)) {
            return [];
        }

        $text = trim($value);

        if ($text === '') {
            return [];
        }

        $violations = self::assertTextSafe($path, $value);

        if (preg_match('/\b(teacher|guru) (will|akan)\b/iu', $text)) {
            $violations[] = [
                'pattern_name' => 'procedural_instruction',
                'matched_text' => $text,
                'field_path' => $path,
                'suggestion' => 'Write learning objectives from student perspective (e.g., "Students will understand...")',
            ];
        }

        return $violations;
    }

    public static function assertSectionPurpose(string $path, mixed $value): array
    {
        $violations = self::assertTextSafe($path, $value);
        if (!is_string($value)) return $violations;

        if (preg_match(self::META_INSTRUCTION_PATTERNS['structural_scaffolding'], $value, $matches)) {
            $violations[] = [
                'pattern_name' => 'structural_scaffolding',
                'matched_text' => $matches[0],
                'field_path' => $path,
                'suggestion' => 'Do not include authoring guidance or meta-guidance in section purpose',
            ];
        }
        return $violations;
    }

    public static function assertAssessmentInstructions(string $path, mixed $value): array
    {
        $violations = self::assertTextSafe($path, $value);
        if (!is_string($value)) return $violations;

        if (preg_match(self::META_INSTRUCTION_PATTERNS['procedural_instruction'], $value, $matches)) {
            $violations[] = [
                'pattern_name' => 'procedural_instruction',
                'matched_text' => $matches[0],
                'field_path' => $path,
                'suggestion' => 'Ensure steps are student-facing activities, not teacher execution checklist',
            ];
        }
        return $violations;
    }

    public static function assertTeacherDeliverySummary(string $path, mixed $value): array
    {
        $violations = self::assertTextSafe($path, $value);
        if (!is_string($value)) return $violations;

        if (self::textLength($value) > 200) {
            $violations[] = [
                'pattern_name' => 'excessive_delivery_summary_length',
                'matched_text' => 'Length > 200',
                'field_path' => $path,
                'suggestion' => 'Must be concise and max 200 chars',
            ];
        }
        if (preg_match('/\b(teacher should|you should teach)\b/iu', $value, $matches)) {
            $violations[] = [
                'pattern_name' => 'procedural_instruction',
                'matched_text' => $matches[0],
                'field_path' => $path,
                'suggestion' => 'Write from student perspective',
            ];
        }
        return $violations;
    }

    private static function assertTextSafe(string $path, mixed $value): array
    {
        if (! is_string($value)) {
            return [];
        }

        $text = trim($value);

        if ($text === '') {
            return [];
        }

        $violations = [];

        foreach (self::FORBIDDEN_PATTERNS as $pattern => $reason) {
            if (preg_match($pattern, $text) === 1) {
                $violations[] = [
                    'pattern_name' => $reason,
                    'matched_text' => $text,
                    'field_path' => $path,
                    'suggestion' => 'Remove api instruction pattern',
                ];
            }
        }

        foreach (self::META_INSTRUCTION_PATTERNS as $name => $pattern) {
            if (preg_match($pattern, $text, $matches) === 1) {
                $violations[] = [
                    'pattern_name' => $name,
                    'matched_text' => $matches[0],
                    'field_path' => $path,
                    'suggestion' => 'Remove pedagogical meta instruction',
                ];
            }
        }

        return $violations;
    }

    private static function assertDraftMaterialText(string $path, mixed $value): array
    {
        if (! is_string($value)) {
            return [];
        }

        $text = trim($value);

        if ($text === '') {
            return [];
        }

        $violations = [];
        foreach (self::DRAFT_SCAFFOLD_PATTERNS as $pattern => $reason) {
            if (preg_match($pattern, $text, $matches) === 1) {
                $violations[] = [
                    'pattern_name' => $reason,
                    'matched_text' => $matches[0],
                    'field_path' => $path,
                    'suggestion' => 'Remove scaffold pattern text',
                ];
            }
        }
        
        return $violations;
    }

    private static function textLength(string $text): int
    {
        $normalized = preg_replace('/\s+/u', ' ', trim($text));

        return mb_strlen($normalized ?? trim($text));
    }
}