<?php

namespace App\MediaGeneration;

final class MediaGenerationErrorCode
{
    public const VALIDATION_FAILED = 'validation_failed';
    public const LLM_CONTRACT_FAILED = 'llm_contract_failed';
    public const PYTHON_SERVICE_UNAVAILABLE = 'python_service_unavailable';
    public const ARTIFACT_INVALID = 'artifact_invalid';
    public const UPLOAD_FAILED = 'upload_failed';
    public const PUBLICATION_FAILED = 'publication_failed';
    public const TEACHER_ROLE_REQUIRED = 'teacher_role_required';
    public const MEDIA_GENERATION_NOT_FOUND = 'media_generation_not_found';

    public static function all(): array
    {
        return [
            self::VALIDATION_FAILED,
            self::LLM_CONTRACT_FAILED,
            self::PYTHON_SERVICE_UNAVAILABLE,
            self::ARTIFACT_INVALID,
            self::UPLOAD_FAILED,
            self::PUBLICATION_FAILED,
            self::TEACHER_ROLE_REQUIRED,
            self::MEDIA_GENERATION_NOT_FOUND,
        ];
    }

    public static function isKnown(?string $code): bool
    {
        return is_string($code) && in_array($code, self::all(), true);
    }

    public static function httpStatus(?string $code): int
    {
        return match ($code) {
            self::VALIDATION_FAILED,
            self::LLM_CONTRACT_FAILED,
            self::ARTIFACT_INVALID => 422,
            self::PYTHON_SERVICE_UNAVAILABLE => 503,
            self::UPLOAD_FAILED,
            self::PUBLICATION_FAILED => 502,
            self::TEACHER_ROLE_REQUIRED => 403,
            self::MEDIA_GENERATION_NOT_FOUND => 404,
            default => 400,
        };
    }

    public static function retryable(?string $code): bool
    {
        return in_array($code, [
            self::LLM_CONTRACT_FAILED,
            self::PYTHON_SERVICE_UNAVAILABLE,
            self::ARTIFACT_INVALID,
            self::UPLOAD_FAILED,
            self::PUBLICATION_FAILED,
        ], true);
    }

    public static function clientMessage(?string $code): string
    {
        return match ($code) {
            self::VALIDATION_FAILED => 'Permintaan media generation tidak valid.',
            self::LLM_CONTRACT_FAILED => 'Sistem belum dapat memahami prompt secara konsisten. Silakan coba lagi.',
            self::PYTHON_SERVICE_UNAVAILABLE => 'Layanan generator sedang tidak tersedia. Silakan coba beberapa saat lagi.',
            self::ARTIFACT_INVALID => 'File hasil generator tidak lolos validasi. Silakan coba lagi.',
            self::UPLOAD_FAILED => 'File berhasil dibuat tetapi gagal diunggah. Silakan coba lagi.',
            self::PUBLICATION_FAILED => 'File berhasil dibuat tetapi gagal dipublikasikan. Silakan coba lagi.',
            self::TEACHER_ROLE_REQUIRED => 'Akses khusus teacher. Anda tidak memiliki izin untuk mengakses fitur ini.',
            self::MEDIA_GENERATION_NOT_FOUND => 'Media generation tidak ditemukan.',
            default => 'Terjadi kegagalan pada media generation.',
        };
    }

    /**
     * @return array{code: string, message: string, retryable: bool}|null
     */
    public static function toClientPayload(?string $code): ?array
    {
        if (! is_string($code) || trim($code) === '') {
            return null;
        }

        $normalizedCode = trim($code);

        return [
            'code' => $normalizedCode,
            'message' => self::clientMessage($normalizedCode),
            'retryable' => self::retryable($normalizedCode),
        ];
    }
}