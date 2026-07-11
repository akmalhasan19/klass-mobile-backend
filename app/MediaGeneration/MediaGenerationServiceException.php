<?php

namespace App\MediaGeneration;

use RuntimeException;

class MediaGenerationServiceException extends RuntimeException
{
    /**
     * @param  array<string, mixed>  $context
     */
    public function __construct(
        string $message,
        protected string $errorCode,
        protected array $context = [],
    ) {
        parent::__construct($message);
    }

    /**
     * @param  array<string, mixed>  $context
     */
    public static function llmContractFailed(string $message, array $context = []): self
    {
        return new self($message, MediaGenerationErrorCode::LLM_CONTRACT_FAILED, $context);
    }

    /**
     * @param  array<string, mixed>  $context
     */
    public static function pythonServiceUnavailable(string $message, array $context = []): self
    {
        return new self($message, MediaGenerationErrorCode::PYTHON_SERVICE_UNAVAILABLE, $context);
    }

    /**
     * @param  array<string, mixed>  $context
     */
    public static function artifactInvalid(string $message, array $context = []): self
    {
        return new self($message, MediaGenerationErrorCode::ARTIFACT_INVALID, $context);
    }

    /**
     * @param  array<string, mixed>  $context
     */
    public static function uploadFailed(string $message, array $context = []): self
    {
        return new self($message, MediaGenerationErrorCode::UPLOAD_FAILED, $context);
    }

    /**
     * @param  array<string, mixed>  $context
     */
    public static function publicationFailed(string $message, array $context = []): self
    {
        return new self($message, MediaGenerationErrorCode::PUBLICATION_FAILED, $context);
    }

    public function errorCode(): string
    {
        return $this->errorCode;
    }

    /**
     * @return array<string, mixed>
     */
    public function context(): array
    {
        return $this->context;
    }
}