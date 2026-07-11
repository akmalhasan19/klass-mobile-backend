<?php

namespace App\Services;

use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationServiceException;
use Exception;
use Illuminate\Http\Client\ConnectionException;
use Illuminate\Http\Client\Factory as HttpFactory;

class LlmAdapterHealthCheckService
{
    public const HEALTH_SCHEMA_VERSION = 'llm_adapter_health.v1';

    public function __construct(protected ?HttpFactory $http = null)
    {
    }

    /**
     * @return array{health: array<string, mixed>, versioned_health: array<string, mixed>}
     */
    public function check(bool $requireAuthConfigured = true): array
    {
        $baseUrl = trim((string) config('services.media_generation.llm_adapter.base_url'));

        if ($baseUrl === '') {
            throw MediaGenerationServiceException::llmContractFailed(
                'LLM adapter service is not configured.',
                ['config' => 'services.media_generation.llm_adapter.base_url']
            );
        }

        $healthPayload = $this->fetchHealthPayload('health', $requireAuthConfigured);
        $versionedHealthPayload = $this->fetchHealthPayload($this->versionedHealthPath(), $requireAuthConfigured);

        $this->assertConsistentContracts($healthPayload, $versionedHealthPayload);

        return [
            'health' => $healthPayload,
            'versioned_health' => $versionedHealthPayload,
        ];
    }

    /**
     * @return array<string, mixed>
     */
    protected function fetchHealthPayload(string $path, bool $requireAuthConfigured): array
    {
        $request = $this->http()
            ->baseUrl(rtrim((string) config('services.media_generation.llm_adapter.base_url'), '/'))
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
            $response = $request->get($path);
        } catch (ConnectionException $exception) {
            throw MediaGenerationServiceException::llmContractFailed(
                'Could not reach the LLM adapter health endpoint.',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'exception' => $exception->getMessage(),
                ]
            );
        }

        $payload = $response->json();

        if (! is_array($payload) || array_is_list($payload)) {
            if ($response->failed()) {
                throw MediaGenerationServiceException::llmContractFailed(
                    'LLM adapter health endpoint returned a non-success status.',
                    [
                        'health_path' => '/' . ltrim($path, '/'),
                        'http_status' => $response->status(),
                        'response_body' => trim($response->body()),
                    ]
                );
            }

            throw new MediaGenerationContractException(
                'LLM adapter health response must be a JSON object.',
                'media_generation_contract_invalid',
                ['health_path' => '/' . ltrim($path, '/')]
            );
        }

        $this->assertHealthPayload($payload, $requireAuthConfigured, $path);

        if ($response->failed()) {
            throw MediaGenerationServiceException::llmContractFailed(
                'LLM adapter health endpoint returned a non-success status.',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'http_status' => $response->status(),
                ]
            );
        }

        return $payload;
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    protected function assertHealthPayload(array $payload, bool $requireAuthConfigured, string $path): void
    {
        if (trim((string) data_get($payload, 'schema_version')) !== self::HEALTH_SCHEMA_VERSION) {
            throw new MediaGenerationContractException(
                'LLM adapter health schema version is invalid.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'schema_version' => data_get($payload, 'schema_version'),
                ]
            );
        }

        if (trim((string) data_get($payload, 'auth.signature_algorithm')) !== InterServiceRequestSigner::SIGNATURE_ALGORITHM) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload reported an unexpected signature algorithm.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'signature_algorithm' => data_get($payload, 'auth.signature_algorithm'),
                ]
            );
        }

        $postgresConfigured = data_get($payload, 'dependencies.postgres.configured');
        $postgresReady = data_get($payload, 'dependencies.postgres.ready');

        if (! is_bool($postgresConfigured) || ! is_bool($postgresReady)) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload must expose Postgres readiness booleans.',
                'media_generation_contract_invalid',
                ['health_path' => '/' . ltrim($path, '/')]
            );
        }

        if (! $postgresConfigured || ! $postgresReady) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload reports Postgres is not ready.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'postgres' => data_get($payload, 'dependencies.postgres'),
                ]
            );
        }

        $this->assertProviderPayload($payload, 'interpretation', 'interpret', $path);
        $this->assertProviderPayload($payload, 'delivery', 'respond', $path);

        $authConfigured = data_get($payload, 'auth.configured');
        $authReady = data_get($payload, 'auth.ready');

        if (! is_bool($authConfigured) || ! is_bool($authReady)) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload must expose auth readiness booleans.',
                'media_generation_contract_invalid',
                ['health_path' => '/' . ltrim($path, '/')]
            );
        }

        if ($requireAuthConfigured && (! $authConfigured || ! $authReady)) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload reports auth is not ready.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'auth' => data_get($payload, 'auth'),
                ]
            );
        }

        if (data_get($payload, 'governance.ready') !== true) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload reports governance is not ready.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'governance' => data_get($payload, 'governance'),
                ]
            );
        }

        $routes = (array) data_get($payload, 'governance.routes', []);
        $routeNames = [];

        foreach ($routes as $route) {
            if (is_array($route) && is_string($route['route'] ?? null)) {
                $routeNames[] = trim((string) $route['route']);
            }
        }

        sort($routeNames);

        if ($routeNames !== ['interpret', 'respond']) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload reported unexpected governance routes.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'routes' => $routeNames,
                ]
            );
        }

        if (data_get($payload, 'status') !== 'ready' || data_get($payload, 'ready') !== true) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload is not ready.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'status' => data_get($payload, 'status'),
                    'ready' => data_get($payload, 'ready'),
                ]
            );
        }
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    protected function assertProviderPayload(array $payload, string $providerKey, string $expectedRoute, string $path): void
    {
        $providerPayload = data_get($payload, 'dependencies.providers.' . $providerKey);

        if (! is_array($providerPayload)) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload is missing provider readiness details.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'provider_key' => $providerKey,
                ]
            );
        }

        if (trim((string) ($providerPayload['route'] ?? '')) !== $expectedRoute) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload reported an unexpected provider route.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'provider_key' => $providerKey,
                    'route' => $providerPayload['route'] ?? null,
                ]
            );
        }

        if (trim((string) ($providerPayload['provider'] ?? '')) === '') {
            throw new MediaGenerationContractException(
                'LLM adapter health payload must report the active provider alias.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'provider_key' => $providerKey,
                ]
            );
        }

        if (($providerPayload['ready'] ?? null) !== true) {
            throw new MediaGenerationContractException(
                'LLM adapter health payload reports an inactive provider route.',
                'media_generation_contract_invalid',
                [
                    'health_path' => '/' . ltrim($path, '/'),
                    'provider_key' => $providerKey,
                    'provider' => $providerPayload,
                ]
            );
        }
    }

    /**
     * @param  array<string, mixed>  $healthPayload
     * @param  array<string, mixed>  $versionedHealthPayload
     */
    protected function assertConsistentContracts(array $healthPayload, array $versionedHealthPayload): void
    {
        foreach ([
            'schema_version',
            'service_name',
            'service_version',
            'dependencies.postgres.ready',
            'dependencies.providers.interpretation.provider',
            'dependencies.providers.delivery.provider',
            'auth.configured',
            'auth.accepted_secret_count',
            'governance.ready',
        ] as $path) {
            if (data_get($healthPayload, $path) !== data_get($versionedHealthPayload, $path)) {
                throw new MediaGenerationContractException(
                    'LLM adapter health endpoints returned inconsistent contract data.',
                    'media_generation_contract_invalid',
                    [
                        'field' => $path,
                        'health' => data_get($healthPayload, $path),
                        'versioned_health' => data_get($versionedHealthPayload, $path),
                    ]
                );
            }
        }
    }

    protected function http(): HttpFactory
    {
        return $this->http ?? app(HttpFactory::class);
    }

    protected function versionedHealthPath(): string
    {
        return ltrim((string) config('services.media_generation.llm_adapter.health_path', '/v1/health'), '/');
    }

    protected function timeoutSeconds(): float
    {
        return (float) config('services.media_generation.interpreter.timeout_seconds', 30);
    }

    protected function connectTimeoutSeconds(): float
    {
        return (float) config('services.media_generation.interpreter.connect_timeout_seconds', 10);
    }

    protected function retryAttempts(): int
    {
        return (int) config('services.media_generation.interpreter.retry_attempts', 2);
    }

    protected function retrySleepMilliseconds(): int
    {
        return (int) config('services.media_generation.interpreter.retry_sleep_milliseconds', 250);
    }
}