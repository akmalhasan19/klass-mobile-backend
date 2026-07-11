<?php

namespace App\MediaGeneration;

use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;

final class MediaGenerationSpecContract
{
    public const VERSION = 'media_generation_spec.v1';

    public static function fromDraft(array $interpretationPayload, array $contentDraftPayload, ?string $preferredOutputType = null): array
    {
        $interpretation = MediaPromptInterpretationSchema::validate($interpretationPayload);
        $contentDraft = MediaContentDraftSchema::validate($contentDraftPayload);
        $exportFormat = self::resolveExportFormat($interpretation, $preferredOutputType);
        $documentMode = $exportFormat === 'pptx' ? 'slide_deck' : 'document';
        $unitType = $exportFormat === 'pptx' ? 'slide' : 'page';
        $sections = array_map(static function (array $section): array {
            return [
                'title' => $section['title'],
                'purpose' => $section['purpose'],
                'body_blocks' => array_map(
                    static fn (array $block): array => [
                        'type' => $block['type'],
                        'content' => $block['content'],
                    ],
                    $section['body_blocks']
                ),
                'emphasis' => $section['emphasis'],
            ];
        }, $contentDraft['sections']);

        $assessmentBlocks = $interpretation['assessment_or_activity_blocks'];
        $styleTone = $interpretation['requested_media_characteristics']['tone']
            ?? $interpretation['constraints']['tone']
            ?? 'clear_and_structured';
        $formatPreferences = $interpretation['requested_media_characteristics']['format_preferences'];

        if ($formatPreferences === []) {
            $formatPreferences = [$exportFormat];
        }

        $contentIntegrity = $contentDraftPayload['content_integrity'] ?? [
            'integrity_score' => 1.0,
            'violations' => [],
            'classification_source' => 'unknown',
            'metadata' => []
        ];

        $threshold = (float) config('content_integrity.classifier_confidence_threshold', 0.75);
        $rejectionStrategy = config('content_integrity.rejection_strategy', 'warn');

        if ($contentIntegrity['integrity_score'] < $threshold) {
            if ($rejectionStrategy === 'strict') {
                throw new MediaGenerationContractException(
                    'Draft content failed integrity score threshold.',
                    'content_integrity_failed',
                    ['content_integrity' => $contentIntegrity]
                );
            } elseif (in_array($rejectionStrategy, ['warn', 'log'], true)) {
                \Illuminate\Support\Facades\Log::warning('Content integrity threshold warning for MediaGeneration', [
                    'integrity_score' => $contentIntegrity['integrity_score'],
                    'threshold' => $threshold,
                    'violations' => $contentIntegrity['violations'] ?? [],
                ]);
            }
        }

        return self::validate([
            'schema_version' => self::VERSION,
            'source_interpretation_schema_version' => $interpretation['schema_version'],
            'export_format' => $exportFormat,
            'title' => $contentDraft['title'],
            'language' => $interpretation['language'],
            'summary' => $contentDraft['summary'],
            'learning_objectives' => $contentDraft['learning_objectives'] !== []
                ? $contentDraft['learning_objectives']
                : $interpretation['learning_objectives'],
            'sections' => $sections,
            'layout_hints' => [
                'document_mode' => $documentMode,
                'visual_density' => $interpretation['requested_media_characteristics']['visual_density'] ?? 'medium',
                'section_count' => count($sections),
                'asset_count' => count($interpretation['assets']),
                'assessment_block_count' => count($assessmentBlocks),
            ],
            'style_hints' => [
                'tone' => $styleTone,
                'audience_level' => $interpretation['target_audience']['level'] ?? 'general',
                'format_preferences' => $formatPreferences,
            ],
            'page_or_slide_structure' => [
                'unit_type' => $unitType,
                'total_units' => 1 + count($sections) + (count($assessmentBlocks) > 0 ? 1 : 0),
                'opening_unit' => true,
                'section_units' => count($sections),
                'closing_unit' => count($assessmentBlocks) > 0,
            ],
            'content_context' => [
                'subject_context' => $interpretation['subject_context'],
                'sub_subject_context' => $interpretation['sub_subject_context'],
                'target_audience' => $interpretation['target_audience'],
            ],
            'content_integrity' => [
                'integrity_score' => (float) $contentIntegrity['integrity_score'],
                'violations' => $contentIntegrity['violations'] ?? [],
                'classification_source' => $contentIntegrity['classification_source'] ?? 'adapter',
                'metadata' => $contentIntegrity['metadata'] ?? null,
            ],
            'assets' => $interpretation['assets'],
            'assessment_or_activity_blocks' => $assessmentBlocks,
            'teacher_delivery_summary' => $contentDraft['teacher_delivery_summary'],
            'contract_versions' => [
                'generator_output_metadata' => MediaArtifactMetadataContract::VERSION,
            ],
        ]);
    }

    public static function fromInterpretation(array $interpretationPayload, ?string $preferredOutputType = null): array
    {
        $interpretation = MediaPromptInterpretationSchema::validate($interpretationPayload);
        $exportFormat = self::resolveExportFormat($interpretation, $preferredOutputType);
        $documentMode = $exportFormat === 'pptx' ? 'slide_deck' : 'document';
        $unitType = $exportFormat === 'pptx' ? 'slide' : 'page';
        $sections = array_map(static function (array $section): array {
            $bodyBlocks = array_map(
                static fn (string $bullet): array => ['type' => 'bullet', 'content' => $bullet],
                $section['bullets']
            );

            if ($bodyBlocks === []) {
                $bodyBlocks[] = [
                    'type' => 'paragraph',
                    'content' => $section['purpose'],
                ];
            }

            return [
                'title' => $section['title'],
                'purpose' => $section['purpose'],
                'body_blocks' => $bodyBlocks,
                'emphasis' => $section['estimated_length'],
            ];
        }, $interpretation['document_blueprint']['sections']);

        $assessmentBlocks = $interpretation['assessment_or_activity_blocks'];
        $styleTone = $interpretation['requested_media_characteristics']['tone']
            ?? $interpretation['constraints']['tone']
            ?? 'clear_and_structured';
        $formatPreferences = $interpretation['requested_media_characteristics']['format_preferences'];

        if ($formatPreferences === []) {
            $formatPreferences = [$exportFormat];
        }

        return self::validate([
            'schema_version' => self::VERSION,
            'source_interpretation_schema_version' => $interpretation['schema_version'],
            'export_format' => $exportFormat,
            'title' => $interpretation['document_blueprint']['title'],
            'language' => $interpretation['language'],
            'summary' => $interpretation['document_blueprint']['summary'],
            'learning_objectives' => $interpretation['learning_objectives'],
            'sections' => $sections,
            'layout_hints' => [
                'document_mode' => $documentMode,
                'visual_density' => $interpretation['requested_media_characteristics']['visual_density'] ?? 'medium',
                'section_count' => count($sections),
                'asset_count' => count($interpretation['assets']),
                'assessment_block_count' => count($assessmentBlocks),
            ],
            'style_hints' => [
                'tone' => $styleTone,
                'audience_level' => $interpretation['target_audience']['level'] ?? 'general',
                'format_preferences' => $formatPreferences,
            ],
            'page_or_slide_structure' => [
                'unit_type' => $unitType,
                'total_units' => 1 + count($sections) + (count($assessmentBlocks) > 0 ? 1 : 0),
                'opening_unit' => true,
                'section_units' => count($sections),
                'closing_unit' => count($assessmentBlocks) > 0,
            ],
            'content_context' => [
                'subject_context' => $interpretation['subject_context'],
                'sub_subject_context' => $interpretation['sub_subject_context'],
                'target_audience' => $interpretation['target_audience'],
            ],
            'content_integrity' => [
                'integrity_score' => 1.0,
                'violations' => [],
                'classification_source' => 'fallback',
                'metadata' => ['synthetic' => true],
            ],
            'assets' => $interpretation['assets'],
            'assessment_or_activity_blocks' => $assessmentBlocks,
            'teacher_delivery_summary' => $interpretation['teacher_delivery_summary'],
            'contract_versions' => [
                'generator_output_metadata' => MediaArtifactMetadataContract::VERSION,
            ],
        ]);
    }

    public static function validate(array $payload): array
    {
        self::assertAllowedKeys($payload, self::topLevelKeys(), 'payload');
        self::assertNestedAllowedKeys($payload);

        $validator = Validator::make($payload, [
            'schema_version' => ['required', 'string', Rule::in([self::VERSION])],
            'source_interpretation_schema_version' => ['required', 'string', Rule::in([MediaPromptInterpretationSchema::VERSION])],
            'export_format' => ['required', 'string', Rule::in(MediaPromptInterpretationSchema::allowedOutputFormats())],
            'title' => ['required', 'string', 'max:200'],
            'language' => ['required', 'string', 'max:32'],
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
            'layout_hints' => ['required', 'array'],
            'layout_hints.document_mode' => ['required', 'string', Rule::in(['document', 'slide_deck'])],
            'layout_hints.visual_density' => ['required', 'string', Rule::in(['low', 'medium', 'high'])],
            'layout_hints.section_count' => ['required', 'integer', 'min:1'],
            'layout_hints.asset_count' => ['required', 'integer', 'min:0'],
            'layout_hints.assessment_block_count' => ['required', 'integer', 'min:0'],
            'style_hints' => ['required', 'array'],
            'style_hints.tone' => ['required', 'string', 'max:100'],
            'style_hints.audience_level' => ['required', 'string', 'max:100'],
            'style_hints.format_preferences' => ['required', 'array', 'min:1'],
            'style_hints.format_preferences.*' => ['string', 'max:100'],
            'page_or_slide_structure' => ['required', 'array'],
            'page_or_slide_structure.unit_type' => ['required', 'string', Rule::in(['page', 'slide'])],
            'page_or_slide_structure.total_units' => ['required', 'integer', 'min:1'],
            'page_or_slide_structure.opening_unit' => ['required', 'boolean'],
            'page_or_slide_structure.section_units' => ['required', 'integer', 'min:1'],
            'page_or_slide_structure.closing_unit' => ['required', 'boolean'],
            'content_context' => ['required', 'array'],
            'content_context.subject_context' => ['nullable', 'array'],
            'content_context.sub_subject_context' => ['nullable', 'array'],
            'content_context.target_audience' => ['nullable', 'array'],
            'content_integrity' => ['required', 'array'],
            'content_integrity.integrity_score' => ['required', 'numeric', 'min:0', 'max:1'],
            'content_integrity.violations' => ['present', 'array'],
            'content_integrity.classification_source' => ['required', 'string'],
            'content_integrity.metadata' => ['nullable', 'array'],
            'assets' => ['present', 'array'],
            'assets.*.type' => ['required', 'string', Rule::in(['text', 'image', 'table', 'chart', 'diagram', 'reference'])],
            'assets.*.description' => ['required', 'string', 'max:500'],
            'assets.*.required' => ['required', 'boolean'],
            'assessment_or_activity_blocks' => ['present', 'array'],
            'assessment_or_activity_blocks.*.title' => ['required', 'string', 'max:200'],
            'assessment_or_activity_blocks.*.type' => ['required', 'string', Rule::in(['assessment', 'activity', 'reflection', 'quiz', 'assignment'])],
            'assessment_or_activity_blocks.*.instructions' => ['required', 'string', 'max:1000'],
            'teacher_delivery_summary' => ['required', 'string', 'max:1000'],
            'contract_versions' => ['required', 'array'],
            'contract_versions.generator_output_metadata' => ['required', 'string', Rule::in([MediaArtifactMetadataContract::VERSION])],
        ]);

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Generation spec payload failed validation.',
                'llm_contract_failed',
                ['errors' => $validator->errors()->toArray()]
            );
        }

        return [
            'schema_version' => self::VERSION,
            'source_interpretation_schema_version' => $payload['source_interpretation_schema_version'],
            'export_format' => trim($payload['export_format']),
            'title' => trim($payload['title']),
            'language' => trim($payload['language']),
            'summary' => trim($payload['summary']),
            'learning_objectives' => array_values($payload['learning_objectives']),
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
            'layout_hints' => [
                'document_mode' => trim($payload['layout_hints']['document_mode']),
                'visual_density' => trim($payload['layout_hints']['visual_density']),
                'section_count' => (int) $payload['layout_hints']['section_count'],
                'asset_count' => (int) $payload['layout_hints']['asset_count'],
                'assessment_block_count' => (int) $payload['layout_hints']['assessment_block_count'],
            ],
            'style_hints' => [
                'tone' => trim($payload['style_hints']['tone']),
                'audience_level' => trim($payload['style_hints']['audience_level']),
                'format_preferences' => array_values($payload['style_hints']['format_preferences']),
            ],
            'page_or_slide_structure' => [
                'unit_type' => trim($payload['page_or_slide_structure']['unit_type']),
                'total_units' => (int) $payload['page_or_slide_structure']['total_units'],
                'opening_unit' => (bool) $payload['page_or_slide_structure']['opening_unit'],
                'section_units' => (int) $payload['page_or_slide_structure']['section_units'],
                'closing_unit' => (bool) $payload['page_or_slide_structure']['closing_unit'],
            ],
            'content_context' => [
                'subject_context' => is_array($payload['content_context']['subject_context']) ? $payload['content_context']['subject_context'] : null,
                'sub_subject_context' => is_array($payload['content_context']['sub_subject_context']) ? $payload['content_context']['sub_subject_context'] : null,
                'target_audience' => is_array($payload['content_context']['target_audience']) ? $payload['content_context']['target_audience'] : null,
            ],
            'content_integrity' => [
                'integrity_score' => (float) $payload['content_integrity']['integrity_score'],
                'violations' => is_array($payload['content_integrity']['violations']) ? array_values($payload['content_integrity']['violations']) : [],
                'classification_source' => trim((string) $payload['content_integrity']['classification_source']),
                'metadata' => is_array($payload['content_integrity']['metadata']) ? $payload['content_integrity']['metadata'] : null,
            ],
            'assets' => array_values($payload['assets']),
            'assessment_or_activity_blocks' => array_values($payload['assessment_or_activity_blocks']),
            'teacher_delivery_summary' => trim($payload['teacher_delivery_summary']),
            'contract_versions' => [
                'generator_output_metadata' => trim($payload['contract_versions']['generator_output_metadata']),
            ],
        ];
    }

    private static function resolveExportFormat(array $interpretation, ?string $preferredOutputType): string
    {
        if ($preferredOutputType !== null && trim($preferredOutputType) !== '') {
            $override = strtolower(trim($preferredOutputType));

            if (! in_array($override, MediaPromptInterpretationSchema::allowedPreferredOutputTypes(), true)) {
                throw new MediaGenerationContractException(
                    'Unsupported export format override.',
                    'llm_contract_failed',
                    ['preferred_output_type' => $preferredOutputType]
                );
            }

            if ($override !== 'auto') {
                return $override;
            }
        }

        $constraintPreferredOutputType = $interpretation['constraints']['preferred_output_type'];

        if ($constraintPreferredOutputType !== 'auto') {
            return $constraintPreferredOutputType;
        }

        return $interpretation['output_type_candidates'][0]['type'];
    }

    private static function assertNestedAllowedKeys(array $payload): void
    {
        if (isset($payload['sections']) && is_array($payload['sections'])) {
            foreach ($payload['sections'] as $index => $section) {
                if (! is_array($section)) {
                    continue;
                }

                self::assertAllowedKeys($section, ['title', 'purpose', 'body_blocks', 'emphasis'], 'sections.' . $index);

                if (isset($section['body_blocks']) && is_array($section['body_blocks'])) {
                    foreach ($section['body_blocks'] as $blockIndex => $block) {
                        if (is_array($block)) {
                            self::assertAllowedKeys($block, ['type', 'content'], 'sections.' . $index . '.body_blocks.' . $blockIndex);
                        }
                    }
                }
            }
        }

        if (isset($payload['layout_hints']) && is_array($payload['layout_hints'])) {
            self::assertAllowedKeys($payload['layout_hints'], ['document_mode', 'visual_density', 'section_count', 'asset_count', 'assessment_block_count'], 'layout_hints');
        }

        if (isset($payload['style_hints']) && is_array($payload['style_hints'])) {
            self::assertAllowedKeys($payload['style_hints'], ['tone', 'audience_level', 'format_preferences'], 'style_hints');
        }

        if (isset($payload['page_or_slide_structure']) && is_array($payload['page_or_slide_structure'])) {
            self::assertAllowedKeys($payload['page_or_slide_structure'], ['unit_type', 'total_units', 'opening_unit', 'section_units', 'closing_unit'], 'page_or_slide_structure');
        }

        if (isset($payload['content_context']) && is_array($payload['content_context'])) {
            self::assertAllowedKeys($payload['content_context'], ['subject_context', 'sub_subject_context', 'target_audience'], 'content_context');
        }

        if (isset($payload['content_integrity']) && is_array($payload['content_integrity'])) {
            self::assertAllowedKeys($payload['content_integrity'], ['integrity_score', 'violations', 'classification_source', 'metadata'], 'content_integrity');
        }

        if (isset($payload['contract_versions']) && is_array($payload['contract_versions'])) {
            self::assertAllowedKeys($payload['contract_versions'], ['generator_output_metadata'], 'contract_versions');
        }
    }

    private static function assertAllowedKeys(array $payload, array $allowedKeys, string $path): void
    {
        $unknownKeys = array_diff(array_keys($payload), $allowedKeys);

        if ($unknownKeys === []) {
            return;
        }

        throw new MediaGenerationContractException(
            'Generation spec payload contains unsupported fields.',
            'llm_contract_failed',
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
            'source_interpretation_schema_version',
            'export_format',
            'title',
            'language',
            'summary',
            'learning_objectives',
            'sections',
            'layout_hints',
            'style_hints',
            'page_or_slide_structure',
            'content_context',
            'content_integrity',
            'assets',
            'assessment_or_activity_blocks',
            'teacher_delivery_summary',
            'contract_versions',
        ];
    }
}