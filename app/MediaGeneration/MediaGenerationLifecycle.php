<?php

namespace App\MediaGeneration;

final class MediaGenerationLifecycle
{
    public const VERSION = 'media_generation_lifecycle.v1';

    public const QUEUED = 'queued';
    public const INTERPRETING = 'interpreting';
    public const CLASSIFIED = 'classified';
    public const GENERATING = 'generating';
    public const UPLOADING = 'uploading';
    public const PUBLISHING = 'publishing';
    public const COMPLETED = 'completed';
    public const FAILED = 'failed';
    public const CANCELLED = 'cancelled';

    public static function minimumStatuses(): array
    {
        return [
            self::QUEUED,
            self::INTERPRETING,
            self::CLASSIFIED,
            self::GENERATING,
            self::UPLOADING,
            self::PUBLISHING,
            self::COMPLETED,
            self::FAILED,
        ];
    }

    public static function all(): array
    {
        return [
            ...self::minimumStatuses(),
            self::CANCELLED,
        ];
    }

    public static function terminalStates(): array
    {
        return [
            self::COMPLETED,
            self::FAILED,
            self::CANCELLED,
        ];
    }

    public static function shouldPrepareCancelledState(): bool
    {
        return true;
    }

    public static function isTerminal(string $status): bool
    {
        self::assertKnownStatus($status);

        return in_array($status, self::terminalStates(), true);
    }

    public static function canTransition(string $from, string $to): bool
    {
        self::assertKnownStatus($from);
        self::assertKnownStatus($to);

        return in_array($to, self::transitions()[$from], true);
    }

    public static function retryBehavior(string $status): string
    {
        self::assertKnownStatus($status);

        return self::statusDefinitions()[$status]['retry_behavior'];
    }

    public static function transitions(): array
    {
        $transitions = [];

        foreach (self::statusDefinitions() as $status => $definition) {
            $transitions[$status] = $definition['next'];
        }

        return $transitions;
    }

    public static function statusDefinitions(): array
    {
        return [
            self::QUEUED => [
                'terminal' => false,
                'retry_behavior' => 'requeue_pending_job',
                'next' => [self::INTERPRETING, self::FAILED, self::CANCELLED],
            ],
            self::INTERPRETING => [
                'terminal' => false,
                'retry_behavior' => 'resume_current_step',
                'next' => [self::CLASSIFIED, self::FAILED, self::CANCELLED],
            ],
            self::CLASSIFIED => [
                'terminal' => false,
                'retry_behavior' => 'continue_to_next_step',
                'next' => [self::GENERATING, self::FAILED, self::CANCELLED],
            ],
            self::GENERATING => [
                'terminal' => false,
                'retry_behavior' => 'resume_current_step',
                'next' => [self::UPLOADING, self::FAILED, self::CANCELLED],
            ],
            self::UPLOADING => [
                'terminal' => false,
                'retry_behavior' => 'resume_current_step',
                'next' => [self::PUBLISHING, self::FAILED, self::CANCELLED],
            ],
            self::PUBLISHING => [
                'terminal' => false,
                'retry_behavior' => 'resume_current_step',
                'next' => [self::COMPLETED, self::FAILED],
            ],
            self::COMPLETED => [
                'terminal' => true,
                'retry_behavior' => 'forbidden',
                'next' => [],
            ],
            self::FAILED => [
                'terminal' => true,
                'retry_behavior' => 'restart_from_interpreting',
                'next' => [],
            ],
            self::CANCELLED => [
                'terminal' => true,
                'retry_behavior' => 'manual_requeue_only',
                'next' => [],
            ],
        ];
    }

    public static function definition(): array
    {
        return [
            'version' => self::VERSION,
            'minimum_statuses' => self::minimumStatuses(),
            'all_statuses' => self::all(),
            'terminal_states' => self::terminalStates(),
            'cancelled_prepared' => self::shouldPrepareCancelledState(),
            'status_definitions' => self::statusDefinitions(),
        ];
    }

    private static function assertKnownStatus(string $status): void
    {
        if (! in_array($status, self::all(), true)) {
            throw new MediaGenerationContractException(
                'Unknown media generation status.',
                'media_generation_status_unknown',
                ['status' => $status]
            );
        }
    }
}