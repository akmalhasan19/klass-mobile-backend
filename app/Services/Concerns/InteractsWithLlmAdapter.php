<?php

namespace App\Services\Concerns;

use Illuminate\Http\Client\Response;

trait InteractsWithLlmAdapter
{
    private const ADAPTER_PROVIDER_HEADER = 'X-Klass-LLM-Provider';
    private const ADAPTER_MODEL_HEADER = 'X-Klass-LLM-Model';
    private const ADAPTER_PRIMARY_PROVIDER_HEADER = 'X-Klass-LLM-Primary-Provider';
    private const ADAPTER_FALLBACK_USED_HEADER = 'X-Klass-LLM-Fallback-Used';
    private const ADAPTER_FALLBACK_REASON_HEADER = 'X-Klass-LLM-Fallback-Reason';

    protected function llmAdapterBaseUrl(string $routeKey): string
    {
        foreach ([
            trim((string) config('services.media_generation.llm_adapter.base_url')),
            trim((string) config("services.media_generation.{$routeKey}.base_url")),
        ] as $candidate) {
            if ($candidate !== '') {
                return rtrim($candidate, '/');
            }
        }

        return '';
    }

    protected function llmAdapterConfigured(string $routeKey): bool
    {
        return $this->llmAdapterBaseUrl($routeKey) !== '';
    }

    /**
     * @return array{provider: string, model: string, primary_provider: string, fallback_used: bool, fallback_reason: ?string, reported_by_adapter: bool}
     */
    protected function resolveLlmAdapterResponseMetadata(Response $response, string $defaultProvider, string $defaultModel): array
    {
        $decodedPayload = $response->json();
        $responseMeta = is_array($decodedPayload) ? data_get($decodedPayload, 'response_meta', []) : [];
        $responseMeta = is_array($responseMeta) ? $responseMeta : [];

        $providerHeader = $this->responseHeaderValue($response, self::ADAPTER_PROVIDER_HEADER);
        $modelHeader = $this->responseHeaderValue($response, self::ADAPTER_MODEL_HEADER);
        $primaryProviderHeader = $this->responseHeaderValue($response, self::ADAPTER_PRIMARY_PROVIDER_HEADER);
        $fallbackReasonHeader = $this->responseHeaderValue($response, self::ADAPTER_FALLBACK_REASON_HEADER);
        $fallbackUsedHeader = $this->parseBooleanHeader(
            $this->responseHeaderValue($response, self::ADAPTER_FALLBACK_USED_HEADER)
        );

        $provider = $this->firstNonEmptyString([
            $providerHeader,
            data_get($responseMeta, 'provider'),
            $defaultProvider,
        ]) ?? trim($defaultProvider);

        $model = $this->firstNonEmptyString([
            $modelHeader,
            data_get($responseMeta, 'model'),
            $defaultModel,
        ]) ?? trim($defaultModel);

        $primaryProvider = $this->firstNonEmptyString([
            $primaryProviderHeader,
            data_get($responseMeta, 'primary_provider'),
            $provider,
            $defaultProvider,
        ]) ?? $provider;

        $fallbackUsed = $fallbackUsedHeader;
        if ($fallbackUsed === null && array_key_exists('fallback_used', $responseMeta)) {
            $fallbackUsed = (bool) data_get($responseMeta, 'fallback_used');
        }

        return [
            'provider' => $provider,
            'model' => $model,
            'primary_provider' => $primaryProvider,
            'fallback_used' => $fallbackUsed ?? false,
            'fallback_reason' => $this->firstNonEmptyString([
                $fallbackReasonHeader,
                data_get($responseMeta, 'fallback_reason'),
            ]),
            'reported_by_adapter' => $providerHeader !== null
                || $modelHeader !== null
                || $primaryProviderHeader !== null
                || array_key_exists('provider', $responseMeta)
                || array_key_exists('model', $responseMeta),
        ];
    }

    protected function responseHeaderValue(Response $response, string $headerName): ?string
    {
        $value = $response->header($headerName);

        if (is_array($value)) {
            $value = $value[0] ?? null;
        }

        if (! is_string($value)) {
            return null;
        }

        $trimmed = trim($value);

        return $trimmed !== '' ? $trimmed : null;
    }

    protected function parseBooleanHeader(?string $value): ?bool
    {
        if ($value === null) {
            return null;
        }

        return match (strtolower($value)) {
            'true', '1', 'yes' => true,
            'false', '0', 'no' => false,
            default => null,
        };
    }

    /**
     * @param  array<int, mixed>  $candidates
     */
    protected function firstNonEmptyString(array $candidates): ?string
    {
        foreach ($candidates as $candidate) {
            if (! is_string($candidate)) {
                continue;
            }

            $trimmed = trim($candidate);

            if ($trimmed !== '') {
                return $trimmed;
            }
        }

        return null;
    }
}