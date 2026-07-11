<?php

namespace App\MediaGeneration;

use RuntimeException;

class MediaGenerationApiException extends RuntimeException
{
    /**
     * @param  array<string, mixed>  $errors
     */
    public function __construct(
        string $message,
        protected string $errorCode,
        protected int $statusCode,
        protected array $errors = [],
    ) {
        parent::__construct($message);
    }

    public static function teacherRoleRequired(): self
    {
        return new self(
            MediaGenerationErrorCode::clientMessage(MediaGenerationErrorCode::TEACHER_ROLE_REQUIRED),
            MediaGenerationErrorCode::TEACHER_ROLE_REQUIRED,
            403,
        );
    }

    public static function notFound(): self
    {
        return new self(
            MediaGenerationErrorCode::clientMessage(MediaGenerationErrorCode::MEDIA_GENERATION_NOT_FOUND),
            MediaGenerationErrorCode::MEDIA_GENERATION_NOT_FOUND,
            404,
        );
    }

    public function errorCode(): string
    {
        return $this->errorCode;
    }

    public function statusCode(): int
    {
        return $this->statusCode;
    }

    /**
     * @return array<string, mixed>
     */
    public function errors(): array
    {
        return $this->errors;
    }
}