<?php

namespace App\Jobs;

use App\Models\MediaGeneration;
use App\Services\MediaGenerationAuditTrailService;
use App\Services\MediaGenerationWorkflowService;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;
use Throwable;

class ProcessMediaGenerationJob implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    public bool $failOnTimeout = true;

    public int $tries = 3;

    public int $timeout = 300;

    public function __construct(public string $mediaGenerationId)
    {
        $this->tries = (int) config('services.media_generation.queue.tries', 3);
        $this->timeout = (int) config('services.media_generation.queue.timeout_seconds', 300);

        $this->onConnection((string) config('services.media_generation.queue.connection', config('queue.default')));
        $this->onQueue((string) config('services.media_generation.queue.name', 'media-generation'));
    }

    public function handle(
        MediaGenerationWorkflowService $workflowService,
        MediaGenerationAuditTrailService $auditTrailService,
    ): void {
        try {
            $workflowService->process($this->mediaGenerationId, $this->currentAttempt(), $this->jobContext());
        } catch (Throwable $throwable) {
            $generation = MediaGeneration::query()->find($this->mediaGenerationId);

            if ($generation !== null) {
                $auditTrailService->recordAttemptFailure(
                    $generation,
                    $throwable,
                    ['step' => 'process_media_generation_job'],
                    $this->currentAttempt(),
                    $this->jobContext(),
                );
            }

            throw $throwable;
        }
    }

    public function failed(Throwable $throwable): void
    {
        $generation = MediaGeneration::query()->find($this->mediaGenerationId);

        if ($generation === null) {
            return;
        }

        app(MediaGenerationAuditTrailService::class)->markFailed(
            $generation,
            $throwable,
            ['step' => 'process_media_generation_job'],
            $this->currentAttempt(),
            $this->jobContext(),
        );
    }

    public function backoff(): int
    {
        return (int) config('services.media_generation.queue.backoff_seconds', 30);
    }

    protected function currentAttempt(): int
    {
        return $this->attempts();
    }

    protected function jobContext(): array
    {
        return [
            'connection' => $this->connection,
            'queue' => $this->queue,
            'tries' => $this->tries,
            'timeout_seconds' => $this->timeout,
            'backoff_seconds' => $this->backoff(),
        ];
    }
}