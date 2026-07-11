<?php

namespace App\Services;

use App\MediaGeneration\MediaArtifactMetadataContract;
use App\MediaGeneration\MediaGenerationErrorCode;
use App\MediaGeneration\MediaGenerationServiceException;
use App\MediaGeneration\MediaGenerationSpecContract;
use App\Models\MediaGeneration;
use Exception;
use Illuminate\Http\Client\ConnectionException;
use Illuminate\Http\Client\Factory as HttpFactory;
use Illuminate\Http\Client\Response;
use JsonException;

class PythonMediaGeneratorClient
{
    public const AUDIT_SCHEMA_VERSION = 'python_media_generator_client.v1';

    public function __construct(protected ?HttpFactory $http = null)
    {
    }

    public function generate(MediaGeneration $generation): MediaGeneration
    {
        $generationSpec = MediaGenerationSpecContract::validate((array) $generation->generation_spec_payload);
        $requestPayload = $this->buildRequestPayload($generation, $generationSpec);
        $encodedPayload = $this->encodePayload($requestPayload);
        $timestamp = (string) now()->timestamp;
        $response = $this->sendGenerationRequest($generation, $encodedPayload, $timestamp);

        if ($response->failed()) {
            $this->throwFailedGenerationRequest($response);
        }

        $decodedPayload = $response->json();
        $artifactMetadata = MediaArtifactMetadataContract::validate(
            $this->extractArtifactMetadata($decodedPayload, $response->body())
        );

        $generation->forceFill([
            'resolved_output_type' => $generationSpec['export_format'],
            'generator_provider' => data_get($artifactMetadata, 'generator.name', $this->provider()),
            'generator_model' => data_get($artifactMetadata, 'generator.version', $this->model()),
            'generator_service_response' => $this->buildAuditPayload(
                generation: $generation,
                requestPayload: $requestPayload,
                encodedPayload: $encodedPayload,
                timestamp: $timestamp,
                response: $response,
                artifactMetadata: $artifactMetadata,
            ),
            'mime_type' => $artifactMetadata['mime_type'],
            'error_code' => null,
            'error_message' => null,
        ])->save();

        return $generation->fresh();
    }

    protected function sendGenerationRequest(MediaGeneration $generation, string $encodedPayload, string $timestamp): Response
    {
        $baseUrl = trim((string) config('services.media_generation.python.base_url'));

        if ($baseUrl === '') {
            throw MediaGenerationServiceException::pythonServiceUnavailable(
                'Python media generator service is not configured.',
                ['config' => 'services.media_generation.python.base_url']
            );
        }

        $request = $this->http()
            ->baseUrl(rtrim($baseUrl, '/'))
            ->acceptJson()
            ->timeout($this->timeoutSeconds())
            ->connectTimeout($this->connectTimeoutSeconds())
            ->withHeaders([
                'Content-Type' => 'application/json',
                'X-Klass-Generation-Id' => $generation->id,
                'X-Klass-Request-Timestamp' => $timestamp,
                'X-Klass-Signature-Algorithm' => 'hmac-sha256',
                'X-Klass-Signature' => $this->signPayload($timestamp, $encodedPayload),
            ])
            ->retry(
                $this->retryAttempts(),
                $this->retrySleepMilliseconds(),
                function (Exception $exception): bool {
                    return $exception instanceof ConnectionException;
                },
                false,
            );

        try {
            return $request->send('POST', $this->generatePath(), ['body' => $encodedPayload]);
        } catch (ConnectionException $exception) {
            throw MediaGenerationServiceException::pythonServiceUnavailable(
                'Could not reach the Python media generator service.',
                ['exception' => $exception->getMessage()]
            );
        }
    }

    protected function buildRequestPayload(MediaGeneration $generation, array $generationSpec): array
    {
        return [
            'generation_id' => $generation->id,
            'generation_spec' => $generationSpec,
            'contracts' => [
                'generation_spec' => MediaGenerationSpecContract::VERSION,
                'artifact_metadata' => MediaArtifactMetadataContract::VERSION,
            ],
        ];
    }

    protected function extractArtifactMetadata(mixed $decodedPayload, string $responseBody): array
    {
        if (is_array($decodedPayload)) {
            foreach (['artifact_metadata', 'data.artifact_metadata'] as $path) {
                $artifactMetadata = data_get($decodedPayload, $path);

                if (is_array($artifactMetadata)) {
                    return $artifactMetadata;
                }
            }

            if (array_key_exists('schema_version', $decodedPayload)
                && array_key_exists('export_format', $decodedPayload)
                && array_key_exists('artifact_locator', $decodedPayload)) {
                return $decodedPayload;
            }
        }

        throw MediaGenerationServiceException::artifactInvalid(
            'Python media generator response did not include artifact metadata.',
            ['response_body' => trim($responseBody)]
        );
    }

    protected function buildAuditPayload(
        MediaGeneration $generation,
        array $requestPayload,
        string $encodedPayload,
        string $timestamp,
        Response $response,
        array $artifactMetadata,
    ): array {
        return [
            'schema_version' => self::AUDIT_SCHEMA_VERSION,
            'provider' => [
                'name' => $this->provider(),
                'model' => $this->model(),
            ],
            'request' => [
                'generation_id' => $generation->id,
                'path' => $this->generatePath(),
                'timestamp' => $timestamp,
                'signature_algorithm' => 'hmac-sha256',
                'body_sha256' => hash('sha256', $encodedPayload),
                'payload' => $requestPayload,
            ],
            'response' => [
                'http_status' => $response->status(),
                'raw_payload' => $response->json() ?? trim($response->body()),
                'artifact_metadata' => $artifactMetadata,
            ],
            'recorded_at' => now()->toISOString(),
        ];
    }

    protected function throwFailedGenerationRequest(Response $response): never
    {
        $decodedPayload = $response->json();
        $pythonErrorCode = is_array($decodedPayload) ? data_get($decodedPayload, 'error.code') : null;
        $pythonErrorMessage = is_array($decodedPayload) ? data_get($decodedPayload, 'error.message') : null;
        $laravelErrorCodeHint = is_array($decodedPayload) ? data_get($decodedPayload, 'error.laravel_error_code_hint') : null;

        $errorCode = $this->mapErrorCodeFromFailedResponse($response, $laravelErrorCodeHint);

        $message = $errorCode === MediaGenerationErrorCode::PYTHON_SERVICE_UNAVAILABLE
            ? 'Python media generator service rejected the request.'
            : 'Python media generator reported an invalid generation request.';

        throw new MediaGenerationServiceException($message, $errorCode, [
            'http_status' => $response->status(),
            'response_body' => trim($response->body()),
            'python_error_code' => is_string($pythonErrorCode) ? trim($pythonErrorCode) : null,
            'python_error_message' => is_string($pythonErrorMessage) ? trim($pythonErrorMessage) : null,
            'python_error_details' => is_array(data_get($decodedPayload, 'error.details'))
                ? data_get($decodedPayload, 'error.details')
                : null,
        ]);
    }

    protected function mapErrorCodeFromFailedResponse(Response $response, mixed $laravelErrorCodeHint): string
    {
        if (is_string($laravelErrorCodeHint) && MediaGenerationErrorCode::isKnown(trim($laravelErrorCodeHint))) {
            return trim($laravelErrorCodeHint);
        }

        return $response->status() >= 500 || $response->status() === 429
            ? MediaGenerationErrorCode::PYTHON_SERVICE_UNAVAILABLE
            : MediaGenerationErrorCode::ARTIFACT_INVALID;
    }

    protected function encodePayload(array $payload): string
    {
        try {
            return json_encode($payload, JSON_THROW_ON_ERROR | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
        } catch (JsonException $exception) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Generation spec payload could not be serialized for the Python service.',
                ['exception' => $exception->getMessage()]
            );
        }
    }

    protected function signPayload(string $timestamp, string $encodedPayload): string
    {
        $sharedSecret = trim((string) config('services.media_generation.python.shared_secret'));

        if ($sharedSecret === '') {
            throw MediaGenerationServiceException::pythonServiceUnavailable(
                'Python media generator shared secret is not configured.',
                ['config' => 'services.media_generation.python.shared_secret']
            );
        }

        return hash_hmac('sha256', $timestamp . '.' . $encodedPayload, $sharedSecret);
    }

    protected function http(): HttpFactory
    {
        return $this->http ?? app(HttpFactory::class);
    }

    protected function provider(): string
    {
        return trim((string) config('services.media_generation.python.provider', 'klass-python'));
    }

    protected function model(): string
    {
        return trim((string) config('services.media_generation.python.model', 'renderer-v1'));
    }

    protected function generatePath(): string
    {
        return ltrim((string) config('services.media_generation.python.generate_path', '/v1/generate'), '/');
    }

    protected function timeoutSeconds(): float
    {
        return (float) config('services.media_generation.python.timeout_seconds', 60);
    }

    protected function connectTimeoutSeconds(): float
    {
        return (float) config('services.media_generation.python.connect_timeout_seconds', 10);
    }

    protected function retryAttempts(): int
    {
        return (int) config('services.media_generation.python.retry_attempts', 2);
    }

    protected function retrySleepMilliseconds(): int
    {
        return (int) config('services.media_generation.python.retry_sleep_milliseconds', 500);
    }
}