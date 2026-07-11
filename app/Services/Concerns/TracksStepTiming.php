<?php

namespace App\Services\Concerns;

use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\MediaGeneration;
use Illuminate\Support\Facades\Log;

trait TracksStepTiming
{
    protected function timedStep(
        MediaGeneration $generation,
        string $stepName,
        callable $callback,
    ): mixed {
        $startHr = hrtime(true);

        try {
            $result = $callback();
            $durationMs = (hrtime(true) - $startHr) / 1_000_000;

            $this->logStepTiming($generation, $stepName, $durationMs, 'success');

            return $result;
        } catch (\Throwable $e) {
            $durationMs = (hrtime(true) - $startHr) / 1_000_000;

            $this->logStepTiming($generation, $stepName, $durationMs, 'failed', $e);

            throw $e;
        }
    }

    protected function logStepTiming(
        MediaGeneration $generation,
        string $stepName,
        float $durationMs,
        string $status,
        ?\Throwable $exception = null,
    ): void {
        $context = [
            'media_generation_id' => $generation->id,
            'step' => $stepName,
            'duration_ms' => round($durationMs, 2),
            'status' => $status,
            'current_lifecycle_status' => $generation->status,
        ];

        if ($exception !== null) {
            $context['error_class'] = get_class($exception);
            $context['error_message'] = mb_substr($exception->getMessage(), 0, 200);
        }

        Log::channel('media_generation')->info('media_generation.step_timing', $context);

        if (app()->bound('sentry')) {
            app('sentry')->getSpan()?->startChild(
                'media_generation.step',
                $stepName,
            )?->finish((int) ($durationMs * 1000));
        }
    }

    protected function computeStepDurationsFromHistory(MediaGeneration $generation): array
    {
        $history = data_get($generation->orchestration_audit_payload, 'status_history', []);
        $durations = [];
        $stepStart = null;
        $currentStep = null;

        foreach ($history as $event) {
            if (($event['event_type'] ?? null) !== 'status_transition') {
                continue;
            }

            $toStatus = $event['to_status'] ?? null;
            $timestamp = $event['at'] ?? null;

            if ($toStatus === null || $timestamp === null) {
                continue;
            }

            $stepName = $this->statusToStepName($toStatus);

            if ($stepName === null) {
                continue;
            }

            if ($currentStep !== null && $stepStart !== null) {
                $stepEnd = \Carbon\CarbonImmutable::parse($timestamp);
                $durations[$currentStep] = ($durations[$currentStep] ?? 0)
                    + $stepStart->diffInMilliseconds($stepEnd);
            }

            $currentStep = $stepName;
            $stepStart = \Carbon\CarbonImmutable::parse($timestamp);
        }

        if ($currentStep !== null && $stepStart !== null) {
            $durations[$currentStep] = ($durations[$currentStep] ?? 0)
                + $stepStart->diffInMilliseconds(now());
        }

        return $durations;
    }

    protected function statusToStepName(string $status): ?string
    {
        return match ($status) {
            MediaGenerationLifecycle::INTERPRETING => 'interpretation',
            MediaGenerationLifecycle::CLASSIFIED => 'classification',
            MediaGenerationLifecycle::GENERATING => 'generation',
            MediaGenerationLifecycle::UPLOADING => 'upload',
            MediaGenerationLifecycle::PUBLISHING => 'publication',
            MediaGenerationLifecycle::COMPLETED => 'delivery',
            default => null,
        };
    }
}
