<?php

namespace App\Services;

use App\MediaGeneration\MediaArtifactMetadataContract;
use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationServiceException;
use App\MediaGeneration\MediaGenerationSpecContract;
use Exception;
use Illuminate\Http\Client\ConnectionException;
use Illuminate\Http\Client\Factory as HttpFactory;

class PythonMediaGeneratorHealthCheckService
{
    public const HEALTH_SCHEMA_VERSION = 'media_generator_health.v1';

    public function __construct(protected ?HttpFactory $http = null)
    {
    }

    /**
     * @return array<string, mixed>
     */
    public function check(bool $requireAuthConfigured = true): array
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
            ->retry(
                $this->retryAttempts(),
                $this->retrySleepMilliseconds(),
                function (Exception $exception): bool {
                    return $exception instanceof ConnectionException;
                },
                false,
            );

        try {
            $response = $request->get($this->healthPath());
        } catch (ConnectionException $exception) {
            throw MediaGenerationServiceException::pythonServiceUnavailable(
                'Could not reach the Python media generator health endpoint.',
                ['exception' => $exception->getMessage()]
            );
        }

        if ($response->failed()) {
            throw MediaGenerationServiceException::pythonServiceUnavailable(
                'Python media generator health endpoint returned a non-success status.',
                [
                    'http_status' => $response->status(),
                    'response_body' => trim($response->body()),
                ]
            );
        }

        $payload = $response->json();

        if (! is_array($payload)) {
            throw new MediaGenerationContractException(
                'Python media generator health response must be a JSON object.',
                'media_generation_contract_invalid',
                ['response_body' => trim($response->body())]
            );
        }

        $this->assertHealthPayload($payload, $requireAuthConfigured);

        return $payload;
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    protected function assertHealthPayload(array $payload, bool $requireAuthConfigured): void
    {
        if (trim((string) data_get($payload, 'schema_version')) !== self::HEALTH_SCHEMA_VERSION) {
            throw new MediaGenerationContractException(
                'Python media generator health schema version is invalid.',
                'media_generation_contract_invalid',
                ['schema_version' => data_get($payload, 'schema_version')]
            );
        }

        if (data_get($payload, 'status') !== 'ok') {
            throw new MediaGenerationContractException(
                'Python media generator health status is not ok.',
                'media_generation_contract_invalid',
                ['status' => data_get($payload, 'status')]
            );
        }

        if (trim((string) data_get($payload, 'auth.signature_algorithm')) !== 'hmac-sha256') {
            throw new MediaGenerationContractException(
                'Python media generator health payload reported an unexpected signature algorithm.',
                'media_generation_contract_invalid',
                ['signature_algorithm' => data_get($payload, 'auth.signature_algorithm')]
            );
        }

        $authConfigured = data_get($payload, 'auth.configured');

        if (! is_bool($authConfigured)) {
            throw new MediaGenerationContractException(
                'Python media generator health payload must expose auth.configured as a boolean.',
                'media_generation_contract_invalid'
            );
        }

        if ($requireAuthConfigured && ! $authConfigured) {
            throw new MediaGenerationContractException(
                'Python media generator health payload reports auth.configured=false.',
                'media_generation_contract_invalid',
                ['health_path' => '/' . $this->healthPath()]
            );
        }

        $supportedFormats = array_values(array_filter(array_map(
            static fn (mixed $value): string => is_string($value) ? strtolower(trim($value)) : '',
            (array) data_get($payload, 'supported_formats', [])
        )));
        sort($supportedFormats);

        if ($supportedFormats !== ['docx', 'pdf', 'pptx']) {
            throw new MediaGenerationContractException(
                'Python media generator health payload reported unsupported formats.',
                'media_generation_contract_invalid',
                ['supported_formats' => $supportedFormats]
            );
        }

        $contracts = [
            'generation_spec' => MediaGenerationSpecContract::VERSION,
            'artifact_metadata' => MediaArtifactMetadataContract::VERSION,
            'response' => 'media_generator_response.v1',
        ];

        foreach ($contracts as $path => $expectedValue) {
            $actualValue = trim((string) data_get($payload, 'contracts.' . $path));

            if ($actualValue !== $expectedValue) {
                throw new MediaGenerationContractException(
                    'Python media generator health payload reported an unexpected contract version.',
                    'media_generation_contract_invalid',
                    [
                        'contract' => $path,
                        'expected' => $expectedValue,
                        'actual' => $actualValue,
                    ]
                );
            }
        }
    }

    protected function http(): HttpFactory
    {
        return $this->http ?? app(HttpFactory::class);
    }

    protected function healthPath(): string
    {
        return ltrim((string) config('services.media_generation.python.health_path', '/v1/health'), '/');
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