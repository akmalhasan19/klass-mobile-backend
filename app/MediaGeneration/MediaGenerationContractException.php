<?php

namespace App\MediaGeneration;

use InvalidArgumentException;

class MediaGenerationContractException extends InvalidArgumentException
{
    public function __construct(
        string $message,
        protected string $errorCode = 'media_generation_contract_invalid',
        protected array $context = []
    ) {
        parent::__construct($message);
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