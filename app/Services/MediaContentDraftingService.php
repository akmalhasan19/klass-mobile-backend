<?php

namespace App\Services;

use App\MediaGeneration\MediaContentDraftRequestContract;
use App\MediaGeneration\MediaContentDraftSchema;
use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationErrorCode;
use App\MediaGeneration\MediaGenerationServiceException;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\Models\MediaGeneration;
use App\Services\Concerns\InteractsWithLlmAdapter;
use Exception;
use Illuminate\Http\Client\ConnectionException;
use Illuminate\Http\Client\Factory as HttpFactory;
use Illuminate\Http\Client\Response;
use JsonException;

class MediaContentDraftingService
{
    use InteractsWithLlmAdapter;

    public function __construct(
        protected ?HttpFactory $http = null,
        protected ?InterServiceRequestSigner $requestSigner = null,
    ) {
    }

    /**
     * @param  array<string, mixed>  $decision
     * @return array{payload: array<string, mixed>, source: string, adapter_metadata: array<string, mixed>|null, fallback_error: array<string, mixed>|null}
     */
    public function draft(MediaGeneration $generation, array $decision): array
    {
        $interpretation = MediaPromptInterpretationSchema::validate((array) $generation->interpretation_payload);
        $resolvedOutputType = MediaGeneration::normalizePreferredOutputType((string) data_get($decision, 'resolved_output_type'));

        if (! $this->shouldCallLlm()) {
            return $this->fallbackResult($interpretation, $resolvedOutputType, 'drafting_service_unconfigured');
        }

        try {
            $response = $this->sendDraftRequest($generation, $decision);

            if ($response->failed()) {
                throw MediaGenerationServiceException::llmContractFailed(
                    'Media content draft service rejected the request.',
                    ['http_status' => $response->status(), 'response_body' => trim($response->body())]
                );
            }

            $rawContent = $this->extractRawContent($response);
            $draftPayload = MediaContentDraftSchema::decodeAndValidate($rawContent, $resolvedOutputType);
            $adapterMetadata = $this->resolveLlmAdapterResponseMetadata(
                $response,
                $this->provider(),
                $this->model(),
            );

            return [
                'payload' => $draftPayload,
                'source' => 'adapter',
                'adapter_metadata' => $adapterMetadata,
                'fallback_error' => null,
            ];
        } catch (Exception $exception) {
            return $this->fallbackResult(
                $interpretation,
                $resolvedOutputType,
                $exception instanceof MediaGenerationContractException || $exception instanceof MediaGenerationServiceException
                    ? $exception->errorCode()
                    : MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                $exception
            );
        }
    }

    protected function shouldCallLlm(): bool
    {
        return $this->llmAdapterConfigured('drafting');
    }

    /**
     * @param  array<string, mixed>  $decision
     */
    protected function sendDraftRequest(MediaGeneration $generation, array $decision): Response
    {
        $baseUrl = $this->llmAdapterBaseUrl('drafting');

        if ($baseUrl === '') {
            throw MediaGenerationServiceException::llmContractFailed(
                'Media content draft service is not configured.',
                ['config' => 'services.media_generation.drafting.base_url']
            );
        }

        $request = $this->http()
            ->baseUrl($baseUrl)
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

        $payload = MediaContentDraftRequestContract::fromGeneration(
            $generation,
            $decision,
            $this->model(),
            MediaContentDraftSchema::llmInstruction(),
        );
        $signedRequest = $this->buildSignedRequestContext($generation, $payload);
        $request = $request->withHeaders($signedRequest['headers']);

        try {
            return $request->send('POST', $this->path(), ['body' => $signedRequest['encoded_payload']]);
        } catch (ConnectionException $exception) {
            throw MediaGenerationServiceException::llmContractFailed(
                'Could not reach the media content draft service.',
                ['exception' => $exception->getMessage()]
            );
        }
    }

    protected function extractRawContent(Response $response): string
    {
        $decodedPayload = $response->json();

        if (is_array($decodedPayload)) {
            foreach (['output_text', 'data.output_text', 'choices.0.message.content', 'choices.0.text', 'content'] as $path) {
                $content = data_get($decodedPayload, $path);

                if (is_string($content) && trim($content) !== '') {
                    return trim($content);
                }
            }

            if (array_key_exists('schema_version', $decodedPayload)) {
                return $this->encodeJson($decodedPayload);
            }
        }

        return trim($response->body());
    }

    /**
     * @param  array<string, mixed>  $interpretation
     * @return array{payload: array<string, mixed>, source: string, adapter_metadata: array<string, mixed>|null, fallback_error: array<string, mixed>|null}
     */
    protected function fallbackResult(
        array $interpretation,
        string $resolvedOutputType,
        string $reasonCode,
        ?Exception $exception = null,
    ): array {
        $fallbackPayload = MediaContentDraftSchema::fallbackFromInterpretation($interpretation, $resolvedOutputType, $reasonCode);
        $fallbackPayload['content_integrity'] = [
            'integrity_score' => 1.0,
            'violations' => [],
            'classification_source' => 'fallback',
            'metadata' => ['synthetic' => true],
        ];

        return [
            'payload' => $fallbackPayload,
            'source' => 'deterministic_fallback',
            'adapter_metadata' => null,
            'fallback_error' => $exception ? [
                'error_code' => $exception instanceof MediaGenerationContractException || $exception instanceof MediaGenerationServiceException
                    ? $exception->errorCode()
                    : $reasonCode,
                'message' => trim($exception->getMessage()),
            ] : [
                'error_code' => $reasonCode,
                'message' => 'Media content drafting fell back to the interpretation outline.',
            ],
        ];
    }

    protected function encodeJson(array $payload): string
    {
        try {
            return json_encode($payload, JSON_THROW_ON_ERROR | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
        } catch (JsonException) {
            return '{}';
        }
    }

    protected function http(): HttpFactory
    {
        return $this->http ?? app(HttpFactory::class);
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array{headers: array<string, string>, encoded_payload: string}
     */
    protected function buildSignedRequestContext(MediaGeneration $generation, array $payload): array
    {
        $sharedSecret = trim((string) config('services.media_generation.llm_adapter.shared_secret'));

        if ($sharedSecret === '') {
            throw MediaGenerationServiceException::llmContractFailed(
                'LLM adapter shared secret is not configured.',
                ['config' => 'services.media_generation.llm_adapter.shared_secret']
            );
        }

        try {
            $encodedPayload = $this->requestSigner()->encodePayload($payload);
        } catch (JsonException $exception) {
            throw MediaGenerationServiceException::llmContractFailed(
                'Media content draft request could not be serialized for the LLM adapter.',
                ['exception' => $exception->getMessage()]
            );
        }

        return $this->requestSigner()->build($sharedSecret, (string) $generation->id, $encodedPayload);
    }

    protected function requestSigner(): InterServiceRequestSigner
    {
        return $this->requestSigner ?? app(InterServiceRequestSigner::class);
    }

    protected function provider(): string
    {
        return trim((string) config('services.media_generation.drafting.provider', 'llm-adapter'));
    }

    protected function model(): string
    {
        return trim((string) config('services.media_generation.drafting.model', 'adapter-managed'));
    }

    protected function path(): string
    {
        return ltrim((string) config('services.media_generation.drafting.path', '/v1/draft'), '/');
    }

    protected function timeoutSeconds(): float
    {
        return (float) config('services.media_generation.drafting.timeout_seconds', 30);
    }

    protected function connectTimeoutSeconds(): float
    {
        return (float) config('services.media_generation.drafting.connect_timeout_seconds', 10);
    }

    protected function retryAttempts(): int
    {
        return (int) config('services.media_generation.drafting.retry_attempts', 2);
    }

    protected function retrySleepMilliseconds(): int
    {
        return (int) config('services.media_generation.drafting.retry_sleep_milliseconds', 250);
    }
}