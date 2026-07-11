<?php

namespace App\Services;

use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaDraftTaxonomyHint;
use App\MediaGeneration\MediaGenerationSpecContract;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\Models\MediaGeneration;

class MediaGenerationDecisionService
{
    public const VERSION = 'media_output_decision.v1';

    private const TYPE_PRIORITY = [
        'pdf' => 0,
        'docx' => 1,
        'pptx' => 2,
    ];

    public function __construct(
        protected ?MediaContentDraftingService $contentDraftingService = null,
    ) {
    }

    public function resolve(MediaGeneration $generation): MediaGeneration
    {
        if (! is_array($generation->interpretation_payload)) {
            throw new MediaGenerationContractException(
                'Interpretation payload must exist before resolving the output type.',
                'llm_contract_failed'
            );
        }

        $interpretation = MediaPromptInterpretationSchema::validate($generation->interpretation_payload);
        $decision = $this->decide($interpretation, $generation->preferred_output_type);
        $contentDraft = $this->resolveContentDraft($generation, $decision, $interpretation);
        $generationSpec = $contentDraft['payload'] !== null
            ? MediaGenerationSpecContract::fromDraft($interpretation, $contentDraft['payload'], $decision['resolved_output_type'])
            : MediaGenerationSpecContract::fromInterpretation($interpretation, $decision['resolved_output_type']);

        $generation->forceFill([
            'resolved_output_type' => $decision['resolved_output_type'],
            'decision_payload' => array_merge($decision, [
                'content_draft' => $contentDraft['metadata'],
            ]),
            'generation_spec_payload' => $generationSpec,
            'error_code' => null,
            'error_message' => null,
        ])->save();

        return $generation->fresh();
    }

    /**
     * @param  array<string, mixed>  $decision
     * @param  array<string, mixed>  $interpretation
     * @return array{payload: array<string, mixed>|null, metadata: array<string, mixed>}
     */
    protected function resolveContentDraft(MediaGeneration $generation, array $decision, array $interpretation): array
    {
        $taxonomyHint = MediaDraftTaxonomyHint::fromGeneration($generation);

        if ($this->contentDraftingService === null) {
            return [
                'payload' => null,
                'metadata' => [
                    'source' => 'interpretation_only',
                    'schema_version' => null,
                    'fallback_error' => null,
                    'adapter_provider' => null,
                    'adapter_model' => null,
                    'adapter_primary_provider' => null,
                    'adapter_fallback_used' => false,
                    'adapter_fallback_reason' => null,
                    'taxonomy_hint' => $taxonomyHint,
                ],
            ];
        }

        $draftResult = $this->contentDraftingService->draft($generation, $decision);
        $adapterMetadata = is_array($draftResult['adapter_metadata'] ?? null) ? $draftResult['adapter_metadata'] : [];

        return [
            'payload' => is_array($draftResult['payload'] ?? null) ? $draftResult['payload'] : null,
            'metadata' => [
                'source' => $draftResult['source'] ?? 'deterministic_fallback',
                'schema_version' => data_get($draftResult, 'payload.schema_version'),
                'fallback_error' => is_array($draftResult['fallback_error'] ?? null) ? $draftResult['fallback_error'] : null,
                'adapter_provider' => data_get($adapterMetadata, 'provider'),
                'adapter_model' => data_get($adapterMetadata, 'model'),
                'adapter_primary_provider' => data_get($adapterMetadata, 'primary_provider'),
                'adapter_fallback_used' => (bool) data_get($adapterMetadata, 'fallback_used', false),
                'adapter_fallback_reason' => data_get($adapterMetadata, 'fallback_reason'),
                'draft_fallback_triggered' => (bool) data_get($draftResult, 'payload.fallback.triggered', false),
                'draft_fallback_reason_code' => data_get($draftResult, 'payload.fallback.reason_code'),
                'taxonomy_hint' => $taxonomyHint,
            ],
        ];
    }

    /**
     * @return array<string, mixed>
     */
    public function decide(array $interpretationPayload, ?string $preferredOutputType = null): array
    {
        $interpretation = MediaPromptInterpretationSchema::validate($interpretationPayload);
        $normalizedPreferredOutputType = MediaGeneration::normalizePreferredOutputType($preferredOutputType);
        $constraintPreferredOutputType = MediaGeneration::normalizePreferredOutputType(
            data_get($interpretation, 'constraints.preferred_output_type')
        );
        $rankedCandidates = $this->rankCandidates($interpretation);

        if ($normalizedPreferredOutputType !== 'auto') {
            return $this->buildDecision(
                preferredOutputType: $normalizedPreferredOutputType,
                resolvedOutputType: $normalizedPreferredOutputType,
                decisionSource: 'teacher_override',
                reasonCode: 'teacher_override',
                reasoning: 'Teacher override selected ' . strtoupper($normalizedPreferredOutputType) . ', so automatic classification was bypassed.',
                constraintPreferredOutputType: $constraintPreferredOutputType,
                rankedCandidates: $rankedCandidates,
                tieBreakerApplied: false,
            );
        }

        if ($constraintPreferredOutputType !== 'auto') {
            return $this->buildDecision(
                preferredOutputType: $normalizedPreferredOutputType,
                resolvedOutputType: $constraintPreferredOutputType,
                decisionSource: 'interpretation_constraint',
                reasonCode: 'interpretation_constraint',
                reasoning: 'Interpretation payload explicitly constrained the output to ' . strtoupper($constraintPreferredOutputType) . '.',
                constraintPreferredOutputType: $constraintPreferredOutputType,
                rankedCandidates: $rankedCandidates,
                tieBreakerApplied: false,
            );
        }

        $selectedCandidate = $rankedCandidates[0];
        $runnerUpCandidate = $rankedCandidates[1] ?? null;
        $tieBreakerApplied = $runnerUpCandidate !== null
            && abs($selectedCandidate['score'] - $runnerUpCandidate['score']) < 0.0001;

        return $this->buildDecision(
            preferredOutputType: $normalizedPreferredOutputType,
            resolvedOutputType: $selectedCandidate['type'],
            decisionSource: 'candidate_ranking',
            reasonCode: $selectedCandidate['reason_code'],
            reasoning: $this->buildRankingReasoning(
                selectedCandidate: $selectedCandidate,
                runnerUpCandidate: $runnerUpCandidate,
                tieBreakerApplied: $tieBreakerApplied,
                interpretationReasoning: $interpretation['resolved_output_type_reasoning'],
            ),
            constraintPreferredOutputType: $constraintPreferredOutputType,
            rankedCandidates: $rankedCandidates,
            tieBreakerApplied: $tieBreakerApplied,
        );
    }

    /**
     * @return array<string, mixed>
     */
    protected function buildDecision(
        string $preferredOutputType,
        string $resolvedOutputType,
        string $decisionSource,
        string $reasonCode,
        string $reasoning,
        string $constraintPreferredOutputType,
        array $rankedCandidates,
        bool $tieBreakerApplied,
    ): array {
        return [
            'schema_version' => self::VERSION,
            'preferred_output_type' => $preferredOutputType,
            'constraint_preferred_output_type' => $constraintPreferredOutputType,
            'resolved_output_type' => $resolvedOutputType,
            'decision_source' => $decisionSource,
            'reason_code' => $reasonCode,
            'reasoning' => $reasoning,
            'ranked_candidates' => $rankedCandidates,
            'tie_breaker_applied' => $tieBreakerApplied,
            'resolved_at' => now()->toISOString(),
        ];
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    protected function rankCandidates(array $interpretation): array
    {
        $scores = [];

        foreach (MediaPromptInterpretationSchema::allowedOutputFormats() as $type) {
            $scores[$type] = [
                'type' => $type,
                'score' => 0.0,
                'candidate_score' => 0.0,
                'reason_code' => 'highest_candidate_score',
                'matched_signals' => [],
                'reasons' => [],
            ];
        }

        foreach ($interpretation['output_type_candidates'] as $candidate) {
            $type = $candidate['type'];
            $candidateScore = round((float) $candidate['score'], 4);

            $scores[$type]['score'] += $candidateScore;
            $scores[$type]['candidate_score'] = $candidateScore;
            $scores[$type]['reasons'][] = 'LLM candidate score ' . number_format($candidateScore, 4, '.', '') . ': ' . $candidate['reason'];
        }

        foreach ($this->keywordSignals($interpretation) as $signal) {
            $type = $signal['type'];

            $scores[$type]['score'] += $signal['weight'];
            $scores[$type]['reason_code'] = $signal['reason_code'];
            $scores[$type]['matched_signals'][] = [
                'reason_code' => $signal['reason_code'],
                'weight' => round($signal['weight'], 4),
                'matched_keyword' => $signal['matched_keyword'],
            ];
            $scores[$type]['reasons'][] = $signal['reason'];
        }

        $rankedCandidates = array_values($scores);

        usort($rankedCandidates, function (array $left, array $right): int {
            if (abs($left['score'] - $right['score']) >= 0.0001) {
                return $right['score'] <=> $left['score'];
            }

            return self::TYPE_PRIORITY[$left['type']] <=> self::TYPE_PRIORITY[$right['type']];
        });

        return array_map(function (array $candidate): array {
            $candidate['score'] = round((float) $candidate['score'], 4);

            return $candidate;
        }, $rankedCandidates);
    }

    /**
     * @return array<int, array{type: string, weight: float, reason_code: string, matched_keyword: string|null, reason: string}>
     */
    protected function keywordSignals(array $interpretation): array
    {
        $signals = [];
        $haystack = $this->decisionHaystack($interpretation);

        foreach ([
            [
                'type' => 'pptx',
                'weight' => 0.35,
                'reason_code' => 'slide_intent_detected',
                'keywords' => ['slide', 'slides', 'deck', 'presentasi', 'presentation', 'slideshow', 'ppt', 'pptx'],
                'reason_template' => 'Keyword "%s" indicates a slide deck or presentation format.',
            ],
            [
                'type' => 'pdf',
                'weight' => 0.25,
                'reason_code' => 'printable_intent_detected',
                'keywords' => ['handout', 'printable', 'print', 'cetak', 'pdf', 'booklet'],
                'reason_template' => 'Keyword "%s" indicates a stable printable document.',
            ],
            [
                'type' => 'docx',
                'weight' => 0.25,
                'reason_code' => 'editable_document_intent_detected',
                'keywords' => ['editable', 'edit', 'docx', 'word', 'worksheet', 'lembar kerja', 'template'],
                'reason_template' => 'Keyword "%s" indicates an editable document workflow.',
            ],
        ] as $signalDefinition) {
            foreach ($signalDefinition['keywords'] as $keyword) {
                if (str_contains($haystack, mb_strtolower($keyword))) {
                    $signals[] = [
                        'type' => $signalDefinition['type'],
                        'weight' => $signalDefinition['weight'],
                        'reason_code' => $signalDefinition['reason_code'],
                        'matched_keyword' => $keyword,
                        'reason' => sprintf($signalDefinition['reason_template'], $keyword),
                    ];

                    break;
                }
            }
        }

        if (data_get($interpretation, 'requested_media_characteristics.visual_density') === 'high'
            && count($interpretation['assets']) > 0) {
            $signals[] = [
                'type' => 'pptx',
                'weight' => 0.12,
                'reason_code' => 'visual_density_signal',
                'matched_keyword' => null,
                'reason' => 'High visual density with explicit assets favors slide-oriented output.',
            ];
        }

        return $signals;
    }

    protected function decisionHaystack(array $interpretation): string
    {
        $segments = [
            $interpretation['teacher_prompt'],
            data_get($interpretation, 'teacher_intent.goal'),
            $interpretation['resolved_output_type_reasoning'],
            data_get($interpretation, 'document_blueprint.title'),
            data_get($interpretation, 'document_blueprint.summary'),
            implode(' ', array_map(
                static fn (array $section): string => implode(' ', [
                    $section['title'],
                    $section['purpose'],
                    implode(' ', $section['bullets']),
                ]),
                $interpretation['document_blueprint']['sections']
            )),
            implode(' ', $interpretation['requested_media_characteristics']['format_preferences']),
        ];

        return mb_strtolower(implode(' ', array_filter($segments, static fn (mixed $segment): bool => is_string($segment) && trim($segment) !== '')));
    }

    protected function buildRankingReasoning(
        array $selectedCandidate,
        ?array $runnerUpCandidate,
        bool $tieBreakerApplied,
        string $interpretationReasoning,
    ): string {
        $reasoning = 'Auto resolution selected ' . strtoupper($selectedCandidate['type'])
            . ' with score ' . number_format((float) $selectedCandidate['score'], 4, '.', '') . '. '
            . implode(' ', array_slice($selectedCandidate['reasons'], 0, 2));

        if ($runnerUpCandidate !== null) {
            $reasoning .= ' Runner-up was ' . strtoupper($runnerUpCandidate['type'])
                . ' at score ' . number_format((float) $runnerUpCandidate['score'], 4, '.', '') . '.';
        }

        if ($tieBreakerApplied) {
            $reasoning .= ' Scores tied, so the deterministic priority order PDF > DOCX > PPTX was applied.';
        }

        return trim($reasoning . ' ' . $interpretationReasoning);
    }
}