<?php

namespace App\Services;

use Illuminate\Support\Str;
use JsonException;

class InterServiceRequestSigner
{
    public const SIGNATURE_ALGORITHM = 'hmac-sha256';

    /**
     * @throws JsonException
     */
    public function encodePayload(array $payload): string
    {
        return json_encode($payload, JSON_THROW_ON_ERROR | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
    }

    /**
     * @return array{request_id: string, timestamp: string, signature_algorithm: string, body_sha256: string, signature: string, encoded_payload: string, headers: array<string, string>}
     */
    public function build(string $sharedSecret, string $generationId, string $encodedPayload, ?string $requestId = null): array
    {
        $resolvedRequestId = trim((string) $requestId) !== '' ? trim((string) $requestId) : (string) Str::uuid();
        $timestamp = (string) now()->timestamp;
        $signature = hash_hmac('sha256', $timestamp . '.' . $encodedPayload, $sharedSecret);

        return [
            'request_id' => $resolvedRequestId,
            'timestamp' => $timestamp,
            'signature_algorithm' => self::SIGNATURE_ALGORITHM,
            'body_sha256' => hash('sha256', $encodedPayload),
            'signature' => $signature,
            'encoded_payload' => $encodedPayload,
            'headers' => [
                'Content-Type' => 'application/json',
                'X-Request-Id' => $resolvedRequestId,
                'X-Klass-Generation-Id' => $generationId,
                'X-Klass-Request-Timestamp' => $timestamp,
                'X-Klass-Signature-Algorithm' => self::SIGNATURE_ALGORITHM,
                'X-Klass-Signature' => $signature,
            ],
        ];
    }
}