<?php

namespace App\Services;

use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationErrorCode;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\MediaGeneration\MediaGenerationServiceException;
use App\Models\MediaGeneration;
use Carbon\CarbonImmutable;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Str;
use Throwable;

class MediaGenerationAuditTrailService
{
    public const VERSION = 'media_generation_orchestration_audit.v1';

    private const HISTORY_LIMIT = 50;

    public function initialize(MediaGeneration $generation): MediaGeneration
    {
        return DB::transaction(function () use ($generation): MediaGeneration {
            $lockedGeneration = $this->lockGeneration($generation);

            if ($this->hasPayload($lockedGeneration)) {
                return $lockedGeneration->fresh();
            }

            $lockedGeneration->forceFill([
                'orchestration_audit_payload' => $this->basePayload($lockedGeneration),
            ])->save();

            return $lockedGeneration->fresh();
        });
    }

    public function transition(
        MediaGeneration $generation,
        string $toStatus,
        array $context = [],
        ?int $attempt = null,
        array $jobContext = [],
    ): MediaGeneration {
        return DB::transaction(function () use ($generation, $toStatus, $context, $attempt, $jobContext): MediaGeneration {
            $lockedGeneration = $this->lockGeneration($generation);
            $payload = $this->payload($lockedGeneration);
            $timestamp = CarbonImmutable::now();
            $fromStatus = (string) $lockedGeneration->status;

            $payload = $this->applyRuntimeMetadata($payload, $lockedGeneration, $timestamp, $attempt, $jobContext);

            if ($fromStatus === $toStatus) {
                $payload['current_status'] = $toStatus;
                $payload['latest_error'] = null;

                $lockedGeneration->forceFill([
                    'orchestration_audit_payload' => $payload,
                    'error_code' => null,
                    'error_message' => null,
                ])->save();

                return $lockedGeneration->fresh();
            }

            if (! MediaGenerationLifecycle::canTransition($fromStatus, $toStatus)) {
                throw new MediaGenerationContractException(
                    'Invalid media generation status transition.',
                    'media_generation_status_transition_invalid',
                    ['from_status' => $fromStatus, 'to_status' => $toStatus]
                );
            }

            $payload = $this->applyTransitionTiming($payload, $lockedGeneration, $fromStatus, $toStatus, $timestamp);
            $payload['current_status'] = $toStatus;
            $payload['resolved_output_type'] = $this->resolveOutputType($lockedGeneration);
            $payload['provider_trace'] = $this->providerTrace($lockedGeneration);
            $payload['latest_error'] = null;
            $payload['status_history'] = $this->appendHistory($payload['status_history'] ?? [], [
                'event_type' => 'status_transition',
                'from_status' => $fromStatus,
                'to_status' => $toStatus,
                'attempt' => $attempt ?? 0,
                'at' => $timestamp->toISOString(),
                'context' => $this->filterContext($context),
            ]);

            $lockedGeneration->forceFill([
                'status' => $toStatus,
                'orchestration_audit_payload' => $payload,
                'error_code' => null,
                'error_message' => null,
            ])->save();

            $this->logStatusTransition($lockedGeneration, $fromStatus, $toStatus, $attempt, $jobContext, $context, $payload);

            return $lockedGeneration->fresh();
        });
    }

    public function recordAttemptFailure(
        MediaGeneration $generation,
        Throwable $throwable,
        array $context = [],
        ?int $attempt = null,
        array $jobContext = [],
    ): MediaGeneration {
        return DB::transaction(function () use ($generation, $throwable, $context, $attempt, $jobContext): MediaGeneration {
            $lockedGeneration = $this->lockGeneration($generation);
            $timestamp = CarbonImmutable::now();
            $payload = $this->applyRuntimeMetadata($this->payload($lockedGeneration), $lockedGeneration, $timestamp, $attempt, $jobContext);
            $error = $this->errorSummary($throwable);

            $payload['latest_error'] = $error;
            $payload['timing']['total_duration_ms'] = $this->totalDurationMs($lockedGeneration, $payload, $timestamp);
            $payload['status_history'] = $this->appendHistory($payload['status_history'] ?? [], [
                'event_type' => 'attempt_failed',
                'status' => $lockedGeneration->status,
                'attempt' => $attempt ?? 0,
                'at' => $timestamp->toISOString(),
                'context' => $this->filterContext($context),
                'error' => $error,
            ]);

            $lockedGeneration->forceFill([
                'orchestration_audit_payload' => $payload,
            ])->save();

            $this->logAttemptFailure($lockedGeneration, $attempt, $jobContext, $context, $error);

            return $lockedGeneration->fresh();
        });
    }

    public function markFailed(
        MediaGeneration $generation,
        Throwable $throwable,
        array $context = [],
        ?int $attempt = null,
        array $jobContext = [],
    ): MediaGeneration {
        return DB::transaction(function () use ($generation, $throwable, $context, $attempt, $jobContext): MediaGeneration {
            $lockedGeneration = $this->lockGeneration($generation);

            if (in_array($lockedGeneration->status, [MediaGenerationLifecycle::COMPLETED, MediaGenerationLifecycle::CANCELLED], true)) {
                return $lockedGeneration->fresh();
            }

            $timestamp = CarbonImmutable::now();
            $fromStatus = (string) $lockedGeneration->status;
            $payload = $this->applyRuntimeMetadata($this->payload($lockedGeneration), $lockedGeneration, $timestamp, $attempt, $jobContext);
            $error = $this->errorSummary($throwable);

            if ($fromStatus !== MediaGenerationLifecycle::FAILED && MediaGenerationLifecycle::canTransition($fromStatus, MediaGenerationLifecycle::FAILED)) {
                $payload = $this->applyTransitionTiming($payload, $lockedGeneration, $fromStatus, MediaGenerationLifecycle::FAILED, $timestamp);
                $payload['status_history'] = $this->appendHistory($payload['status_history'] ?? [], [
                    'event_type' => 'status_transition',
                    'from_status' => $fromStatus,
                    'to_status' => MediaGenerationLifecycle::FAILED,
                    'attempt' => $attempt ?? 0,
                    'at' => $timestamp->toISOString(),
                    'context' => $this->filterContext($context),
                    'error' => $error,
                ]);
            }

            $payload['current_status'] = MediaGenerationLifecycle::FAILED;
            $payload['resolved_output_type'] = $this->resolveOutputType($lockedGeneration);
            $payload['provider_trace'] = $this->providerTrace($lockedGeneration);
            $payload['latest_error'] = $error;

            $lockedGeneration->forceFill([
                'status' => MediaGenerationLifecycle::FAILED,
                'error_code' => $error['error_code'],
                'error_message' => $this->sanitizeMessage($throwable->getMessage()),
                'orchestration_audit_payload' => $payload,
            ])->save();

            $this->logFinalFailure($lockedGeneration, $attempt, $jobContext, $context, $error);

            return $lockedGeneration->fresh();
        });
    }

    protected function lockGeneration(MediaGeneration $generation): MediaGeneration
    {
        return MediaGeneration::query()->lockForUpdate()->findOrFail($generation->getKey());
    }

    protected function hasPayload(MediaGeneration $generation): bool
    {
        return is_array($generation->orchestration_audit_payload)
            && data_get($generation->orchestration_audit_payload, 'schema_version') === self::VERSION
            && is_array(data_get($generation->orchestration_audit_payload, 'status_history'));
    }

    protected function payload(MediaGeneration $generation): array
    {
        if (! $this->hasPayload($generation)) {
            return $this->basePayload($generation);
        }

        $payload = $generation->orchestration_audit_payload;
        $timing = is_array(data_get($payload, 'timing')) ? data_get($payload, 'timing') : [];
        $job = is_array(data_get($payload, 'job')) ? data_get($payload, 'job') : [];

        $payload['timing'] = array_replace([
            'queued_at' => $generation->created_at?->toISOString(),
            'processing_started_at' => null,
            'last_transition_at' => $generation->created_at?->toISOString(),
            'completed_at' => null,
            'total_duration_ms' => null,
            'status_durations_ms' => [],
        ], $timing);

        $payload['job'] = array_replace([
            'connection' => null,
            'queue' => null,
            'tries' => null,
            'timeout_seconds' => null,
            'backoff_seconds' => null,
            'attempt' => 0,
            'last_run_at' => null,
        ], $job);

        $payload['schema_version'] = self::VERSION;
        $payload['current_status'] = $generation->status;
        $payload['resolved_output_type'] = $this->resolveOutputType($generation);
        $payload['provider_trace'] = $this->providerTrace($generation);
        $payload['status_history'] = array_values(is_array(data_get($payload, 'status_history')) ? data_get($payload, 'status_history') : []);
        $payload['latest_error'] = is_array(data_get($payload, 'latest_error')) ? data_get($payload, 'latest_error') : null;

        return $payload;
    }

    protected function basePayload(MediaGeneration $generation): array
    {
        $createdAt = $generation->created_at?->toISOString() ?? CarbonImmutable::now()->toISOString();

        return [
            'schema_version' => self::VERSION,
            'current_status' => $generation->status,
            'resolved_output_type' => $this->resolveOutputType($generation),
            'provider_trace' => $this->providerTrace($generation),
            'job' => [
                'connection' => null,
                'queue' => null,
                'tries' => null,
                'timeout_seconds' => null,
                'backoff_seconds' => null,
                'attempt' => 0,
                'last_run_at' => null,
            ],
            'timing' => [
                'queued_at' => $createdAt,
                'processing_started_at' => null,
                'last_transition_at' => $createdAt,
                'completed_at' => null,
                'total_duration_ms' => null,
                'status_durations_ms' => [],
            ],
            'latest_error' => null,
            'status_history' => [[
                'event_type' => 'status_transition',
                'from_status' => null,
                'to_status' => $generation->status,
                'attempt' => 0,
                'at' => $createdAt,
                'context' => ['reason' => 'generation_created'],
            ]],
        ];
    }

    protected function applyRuntimeMetadata(
        array $payload,
        MediaGeneration $generation,
        CarbonImmutable $timestamp,
        ?int $attempt,
        array $jobContext,
    ): array {
        $payload['job'] = array_replace($payload['job'] ?? [], [
            'connection' => data_get($jobContext, 'connection'),
            'queue' => data_get($jobContext, 'queue'),
            'tries' => data_get($jobContext, 'tries'),
            'timeout_seconds' => data_get($jobContext, 'timeout_seconds'),
            'backoff_seconds' => data_get($jobContext, 'backoff_seconds'),
            'attempt' => $attempt ?? data_get($payload, 'job.attempt', 0),
            'last_run_at' => $timestamp->toISOString(),
        ]);
        $payload['provider_trace'] = $this->providerTrace($generation);
        $payload['resolved_output_type'] = $this->resolveOutputType($generation);

        return $payload;
    }

    protected function applyTransitionTiming(
        array $payload,
        MediaGeneration $generation,
        string $fromStatus,
        string $toStatus,
        CarbonImmutable $timestamp,
    ): array {
        $statusDurations = is_array(data_get($payload, 'timing.status_durations_ms'))
            ? data_get($payload, 'timing.status_durations_ms')
            : [];

        $lastTransitionAt = $this->parseTimestamp((string) data_get($payload, 'timing.last_transition_at'))
            ?? $generation->created_at?->toImmutable()
            ?? $timestamp;

        $statusDurations[$fromStatus] = ($statusDurations[$fromStatus] ?? 0) + $lastTransitionAt->diffInMilliseconds($timestamp);

        $processingStartedAt = data_get($payload, 'timing.processing_started_at');
        if ($processingStartedAt === null && $toStatus !== MediaGenerationLifecycle::QUEUED) {
            $processingStartedAt = $timestamp->toISOString();
        }

        $payload['timing'] = array_replace($payload['timing'] ?? [], [
            'queued_at' => data_get($payload, 'timing.queued_at') ?? $generation->created_at?->toISOString() ?? $timestamp->toISOString(),
            'processing_started_at' => $processingStartedAt,
            'last_transition_at' => $timestamp->toISOString(),
            'completed_at' => MediaGenerationLifecycle::isTerminal($toStatus)
                ? $timestamp->toISOString()
                : data_get($payload, 'timing.completed_at'),
            'total_duration_ms' => $this->totalDurationMs($generation, $payload, $timestamp),
            'status_durations_ms' => $statusDurations,
        ]);

        return $payload;
    }

    protected function totalDurationMs(MediaGeneration $generation, array $payload, CarbonImmutable $timestamp): int
    {
        $queuedAt = $this->parseTimestamp((string) data_get($payload, 'timing.queued_at'))
            ?? $generation->created_at?->toImmutable()
            ?? $timestamp;

        return $queuedAt->diffInMilliseconds($timestamp);
    }

    protected function parseTimestamp(?string $value): ?CarbonImmutable
    {
        if (! is_string($value) || trim($value) === '') {
            return null;
        }

        try {
            return CarbonImmutable::parse($value);
        } catch (Throwable) {
            return null;
        }
    }

    protected function appendHistory(array $history, array $event): array
    {
        $history[] = $event;

        if (count($history) <= self::HISTORY_LIMIT) {
            return array_values($history);
        }

        return array_values(array_slice($history, -1 * self::HISTORY_LIMIT));
    }

    protected function providerTrace(MediaGeneration $generation): array
    {
        return [
            'interpretation' => [
                'name' => $generation->llm_provider,
                'model' => $generation->llm_model,
            ],
            'generator' => [
                'name' => $generation->generator_provider,
                'model' => $generation->generator_model,
            ],
            'delivery' => [
                'name' => data_get($generation->delivery_payload, 'response_meta.provider'),
                'model' => data_get($generation->delivery_payload, 'response_meta.model'),
            ],
        ];
    }

    protected function resolveOutputType(MediaGeneration $generation): ?string
    {
        foreach ([
            $generation->resolved_output_type,
            data_get($generation->generation_spec_payload, 'export_format'),
            $generation->preferred_output_type,
        ] as $candidate) {
            if (is_string($candidate) && trim($candidate) !== '') {
                return trim($candidate);
            }
        }

        return null;
    }

    protected function errorSummary(Throwable $throwable): array
    {
        $errorCode = match (true) {
            $throwable instanceof MediaGenerationServiceException => $throwable->errorCode(),
            $throwable instanceof MediaGenerationContractException => $throwable->errorCode(),
            default => MediaGenerationErrorCode::PUBLICATION_FAILED,
        };

        return [
            'error_code' => $errorCode,
            'error_class' => ltrim($throwable::class, '\\'),
            'message' => $this->sanitizeMessage($throwable->getMessage()),
            'retryable' => MediaGenerationErrorCode::retryable($errorCode),
            'safe_context' => $this->safeThrowableContext($throwable),
        ];
    }

    protected function safeThrowableContext(Throwable $throwable): array
    {
        $context = [];

        if ($throwable instanceof MediaGenerationServiceException || $throwable instanceof MediaGenerationContractException) {
            foreach (['http_status', 'config', 'kind', 'adapter_provider', 'adapter_model', 'adapter_primary_provider', 'adapter_fallback_used', 'adapter_fallback_reason'] as $key) {
                $value = data_get($throwable->context(), $key);

                if (is_scalar($value)) {
                    $context[$key] = $value;
                }
            }
        }

        return $context;
    }

    protected function sanitizeMessage(string $message): string
    {
        return Str::of($message)
            ->replaceMatches('/\s+/u', ' ')
            ->trim()
            ->limit(240, '')
            ->toString();
    }

    protected function filterContext(array $context, int $depth = 0): array
    {
        if ($depth > 1) {
            return [];
        }

        $filtered = [];

        foreach ($context as $key => $value) {
            if ($value === null) {
                continue;
            }

            if (is_bool($value) || is_int($value) || is_float($value)) {
                $filtered[(string) $key] = $value;
                continue;
            }

            if (is_string($value)) {
                $trimmed = trim($value);

                if ($trimmed !== '') {
                    $filtered[(string) $key] = Str::limit($this->sanitizeMessage($trimmed), 160, '');
                }

                continue;
            }

            if (! is_array($value)) {
                continue;
            }

            if (array_is_list($value)) {
                $items = [];

                foreach ($value as $item) {
                    if (is_bool($item) || is_int($item) || is_float($item)) {
                        $items[] = $item;
                    } elseif (is_string($item) && trim($item) !== '') {
                        $items[] = Str::limit($this->sanitizeMessage($item), 80, '');
                    }
                }

                if ($items !== []) {
                    $filtered[(string) $key] = array_slice($items, 0, 8);
                }

                continue;
            }

            $nested = $this->filterContext($value, $depth + 1);

            if ($nested !== []) {
                $filtered[(string) $key] = $nested;
            }
        }

        return $filtered;
    }

    protected function logStatusTransition(
        MediaGeneration $generation,
        string $fromStatus,
        string $toStatus,
        ?int $attempt,
        array $jobContext,
        array $context,
        array $payload,
    ): void {
        Log::channel('media_generation')->info('media_generation.status_transition', [
            'media_generation_id' => $generation->id,
            'teacher_id' => $generation->teacher_id,
            'from_status' => $fromStatus,
            'to_status' => $toStatus,
            'attempt' => $attempt ?? 0,
            'queue' => data_get($jobContext, 'queue'),
            'connection' => data_get($jobContext, 'connection'),
            'resolved_output_type' => $payload['resolved_output_type'] ?? null,
            'provider_trace' => $payload['provider_trace'] ?? [],
            'duration_ms' => data_get($payload, 'timing.total_duration_ms'),
            'context' => $this->filterContext($context),
        ]);
    }

    protected function logAttemptFailure(
        MediaGeneration $generation,
        ?int $attempt,
        array $jobContext,
        array $context,
        array $error,
    ): void {
        Log::channel('media_generation')->warning('media_generation.attempt_failed', [
            'media_generation_id' => $generation->id,
            'teacher_id' => $generation->teacher_id,
            'status' => $generation->status,
            'attempt' => $attempt ?? 0,
            'queue' => data_get($jobContext, 'queue'),
            'connection' => data_get($jobContext, 'connection'),
            'resolved_output_type' => $this->resolveOutputType($generation),
            'error' => $error,
            'context' => $this->filterContext($context),
        ]);
    }

    protected function logFinalFailure(
        MediaGeneration $generation,
        ?int $attempt,
        array $jobContext,
        array $context,
        array $error,
    ): void {
        Log::channel('media_generation')->error('media_generation.failed', [
            'media_generation_id' => $generation->id,
            'teacher_id' => $generation->teacher_id,
            'status' => $generation->status,
            'attempt' => $attempt ?? 0,
            'queue' => data_get($jobContext, 'queue'),
            'connection' => data_get($jobContext, 'connection'),
            'resolved_output_type' => $this->resolveOutputType($generation),
            'duration_ms' => data_get($generation->orchestration_audit_payload, 'timing.total_duration_ms'),
            'error' => $error,
            'context' => $this->filterContext($context),
        ]);
    }
}