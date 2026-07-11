<?php

namespace App\Services;

use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\MediaGeneration;
use App\Services\Concerns\TracksStepTiming;

class MediaGenerationWorkflowService
{
    use TracksStepTiming;

    private const STATUS_ORDER = [
        MediaGenerationLifecycle::QUEUED => 0,
        MediaGenerationLifecycle::INTERPRETING => 1,
        MediaGenerationLifecycle::CLASSIFIED => 2,
        MediaGenerationLifecycle::GENERATING => 3,
        MediaGenerationLifecycle::UPLOADING => 4,
        MediaGenerationLifecycle::PUBLISHING => 5,
        MediaGenerationLifecycle::COMPLETED => 6,
        MediaGenerationLifecycle::FAILED => 7,
        MediaGenerationLifecycle::CANCELLED => 8,
    ];

    public function __construct(
        protected MediaPromptInterpretationService $interpretationService,
        protected MediaGenerationDecisionService $decisionService,
        protected PythonMediaGeneratorClient $pythonMediaGeneratorClient,
        protected MediaPublicationService $publicationService,
        protected MediaDeliveryResponseService $deliveryResponseService,
        protected MediaGenerationAuditTrailService $auditTrailService,
    ) {
    }

    public function process(string $generationId, ?int $attempt = null, array $jobContext = []): MediaGeneration
    {
        $generation = $this->auditTrailService->initialize($this->loadGeneration($generationId));

        if ($generation->isTerminal()) {
            return $generation;
        }

        $attempt ??= 1;

        $generation = $this->timedStep($generation, 'ensure_classified', fn () => $this->ensureClassified($generation, $attempt, $jobContext));
        $generation = $this->timedStep($generation, 'ensure_generated', fn () => $this->ensureGenerated($generation, $attempt, $jobContext));
        $generation = $this->timedStep($generation, 'ensure_published', fn () => $this->ensurePublished($generation, $attempt, $jobContext));

        return $this->timedStep($generation, 'ensure_completed', fn () => $this->ensureCompleted($generation, $attempt, $jobContext));
    }

    protected function ensureClassified(MediaGeneration $generation, int $attempt, array $jobContext): MediaGeneration
    {
        if (! $this->hasClassification($generation)) {
            if ($this->statusBefore($generation, MediaGenerationLifecycle::INTERPRETING)) {
                $generation = $this->auditTrailService->transition(
                    $generation,
                    MediaGenerationLifecycle::INTERPRETING,
                    [
                        'step' => 'interpretation',
                        'provider' => config('services.media_generation.interpreter.provider'),
                        'model' => config('services.media_generation.interpreter.model'),
                    ],
                    $attempt,
                    $jobContext,
                );
            }

            if (! $this->hasInterpretation($generation)) {
                $generation = $this->interpretationService->interpret($generation);
            }

            if (! $this->hasClassification($generation)) {
                $generation = $this->decisionService->resolve($generation);
            }
        }

        if ($this->statusBefore($generation, MediaGenerationLifecycle::CLASSIFIED)) {
            $generation = $this->auditTrailService->transition(
                $generation,
                MediaGenerationLifecycle::CLASSIFIED,
                [
                    'step' => 'classification',
                    'decision_source' => data_get($generation->decision_payload, 'decision_source'),
                    'reason_code' => data_get($generation->decision_payload, 'reason_code'),
                    'content_draft_source' => data_get($generation->decision_payload, 'content_draft.source'),
                    'resolved_output_type' => $generation->resolved_output_type,
                ],
                $attempt,
                $jobContext,
            );
        }

        return $generation;
    }

    protected function ensureGenerated(MediaGeneration $generation, int $attempt, array $jobContext): MediaGeneration
    {
        if (! $this->hasGeneratedArtifact($generation)) {
            if ($this->statusBefore($generation, MediaGenerationLifecycle::GENERATING)) {
                $generation = $this->auditTrailService->transition(
                    $generation,
                    MediaGenerationLifecycle::GENERATING,
                    [
                        'step' => 'generation',
                        'resolved_output_type' => $generation->resolved_output_type,
                    ],
                    $attempt,
                    $jobContext,
                );
            }

            $generation = $this->pythonMediaGeneratorClient->generate($generation);
        }

        if ($this->statusBefore($generation, MediaGenerationLifecycle::UPLOADING)) {
            $generation = $this->auditTrailService->transition(
                $generation,
                MediaGenerationLifecycle::UPLOADING,
                [
                    'step' => 'artifact_upload',
                    'generator_provider' => $generation->generator_provider,
                    'generator_model' => $generation->generator_model,
                    'mime_type' => $generation->mime_type,
                ],
                $attempt,
                $jobContext,
            );
        }

        return $generation;
    }

    protected function ensurePublished(MediaGeneration $generation, int $attempt, array $jobContext): MediaGeneration
    {
        if ($this->hasPublicationEntities($generation)) {
            if ($this->statusBefore($generation, MediaGenerationLifecycle::PUBLISHING)) {
                $generation = $this->auditTrailService->transition(
                    $generation,
                    MediaGenerationLifecycle::PUBLISHING,
                    [
                        'step' => 'publication',
                        'reused_publication_entities' => true,
                    ],
                    $attempt,
                    $jobContext,
                );
            }

            return $generation;
        }

        $generation = $this->publicationService->publish(
            $generation,
            function (MediaGeneration $publishingGeneration, array $preparedArtifact) use ($attempt, $jobContext): MediaGeneration {
                if (! $this->statusBefore($publishingGeneration, MediaGenerationLifecycle::PUBLISHING)) {
                    return $publishingGeneration;
                }

                return $this->auditTrailService->transition(
                    $publishingGeneration,
                    MediaGenerationLifecycle::PUBLISHING,
                    [
                        'step' => 'publication',
                        'artifact_uploaded' => is_string($preparedArtifact['storage_path'] ?? null) && trim((string) $preparedArtifact['storage_path']) !== '',
                        'thumbnail_available' => is_string($preparedArtifact['thumbnail_url'] ?? null) && trim((string) $preparedArtifact['thumbnail_url']) !== '',
                    ],
                    $attempt,
                    $jobContext,
                );
            }
        );

        return $generation;
    }

    protected function ensureCompleted(MediaGeneration $generation, int $attempt, array $jobContext): MediaGeneration
    {
        if (! $this->hasFinalDeliveryPayload($generation)) {
            $generation = $this->deliveryResponseService->compose($generation);
        }

        if ($generation->status !== MediaGenerationLifecycle::COMPLETED) {
            $generation = $this->auditTrailService->transition(
                $generation,
                MediaGenerationLifecycle::COMPLETED,
                [
                    'step' => 'delivery',
                    'delivery_fallback' => (bool) data_get($generation->delivery_payload, 'fallback.triggered', false),
                    'delivery_provider' => data_get($generation->delivery_payload, 'response_meta.provider'),
                    'delivery_model' => data_get($generation->delivery_payload, 'response_meta.model'),
                ],
                $attempt,
                $jobContext,
            );
        }

        return $generation;
    }

    protected function loadGeneration(string $generationId): MediaGeneration
    {
        return MediaGeneration::query()
            ->with(['teacher', 'subject', 'subSubject.subject', 'topic', 'content', 'recommendedProject'])
            ->findOrFail($generationId);
    }

    protected function hasInterpretation(MediaGeneration $generation): bool
    {
        return is_array($generation->interpretation_payload)
            && trim((string) data_get($generation->interpretation_payload, 'schema_version')) !== '';
    }

    protected function hasClassification(MediaGeneration $generation): bool
    {
        return $this->hasInterpretation($generation)
            && is_array($generation->generation_spec_payload)
            && trim((string) data_get($generation->generation_spec_payload, 'schema_version')) !== ''
            && is_string($generation->resolved_output_type)
            && trim($generation->resolved_output_type) !== '';
    }

    protected function hasGeneratedArtifact(MediaGeneration $generation): bool
    {
        return is_array(data_get($generation->generator_service_response, 'response.artifact_metadata'))
            || is_array(data_get($generation->generator_service_response, 'artifact_metadata'));
    }

    protected function hasPublicationEntities(MediaGeneration $generation): bool
    {
        return $generation->topic_id !== null
            && $generation->content_id !== null
            && $generation->recommended_project_id !== null;
    }

    protected function hasFinalDeliveryPayload(MediaGeneration $generation): bool
    {
        return is_array($generation->delivery_payload)
            && trim((string) data_get($generation->delivery_payload, 'schema_version')) !== '';
    }

    protected function statusBefore(MediaGeneration $generation, string $checkpoint): bool
    {
        return self::STATUS_ORDER[$generation->status] < self::STATUS_ORDER[$checkpoint];
    }
}