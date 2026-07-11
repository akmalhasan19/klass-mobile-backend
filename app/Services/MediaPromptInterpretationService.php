<?php

namespace App\Services;

use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationServiceException;
use App\MediaGeneration\MediaPromptInterpretationRequestContract;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\Models\MediaGeneration;
use App\Services\Concerns\InteractsWithLlmAdapter;
use Exception;
use Illuminate\Http\Client\ConnectionException;
use Illuminate\Http\Client\Factory as HttpFactory;
use Illuminate\Http\Client\Response;
use JsonException;

class MediaPromptInterpretationService
{
    use InteractsWithLlmAdapter;

    public const AUDIT_SCHEMA_VERSION = 'media_prompt_interpretation_audit.v1';

    public function __construct(
        protected ?HttpFactory $http = null,
        protected ?InterServiceRequestSigner $requestSigner = null,
        protected ?MediaPromptTaxonomyInferenceService $taxonomyInferenceService = null,
    )
    {
    }

    public function interpret(MediaGeneration $generation): MediaGeneration
    {
        $requestContext = $this->buildRequestContext($generation);
        $requestPayload = $requestContext['payload'];
        $taxonomyInference = $requestContext['taxonomy_inference'];
        $signedRequest = $this->buildSignedRequestContext($generation, $requestPayload);
        $response = $this->sendInterpretationRequest($signedRequest);

        if ($response->failed()) {
            $this->throwFailedInterpretationRequest($response);
        }

        $rawContent = $this->extractRawInterpretationContent($response);
        $normalization = $this->normalizeInterpretationPayload($generation, $rawContent, $taxonomyInference);
        $adapterMetadata = $this->resolveLlmAdapterResponseMetadata(
            $response,
            $this->provider(),
            $this->model(),
        );

        $generation->forceFill([
            'llm_provider' => $adapterMetadata['provider'],
            'llm_model' => $adapterMetadata['model'],
            'interpretation_payload' => $normalization['payload'],
            'interpretation_audit_payload' => $this->buildAuditPayload(
                generation: $generation,
                providerMetadata: $adapterMetadata,
                requestPayload: $requestPayload,
                requestMeta: $this->buildRequestMeta($signedRequest),
                response: $response,
                rawContent: $rawContent,
                normalizedPayload: $normalization['payload'],
                usedFallback: $normalization['used_fallback'],
                fallbackError: $normalization['fallback_error'],
                taxonomyInference: $taxonomyInference,
            ),
            'error_code' => null,
            'error_message' => null,
        ])->save();

        return $generation->fresh(['subject', 'subSubject.subject']);
    }

    /**
     * @param  array{headers: array<string, string>, encoded_payload: string}  $signedRequest
     */
    protected function sendInterpretationRequest(array $signedRequest): Response
    {
        $baseUrl = $this->llmAdapterBaseUrl('interpreter');

        if ($baseUrl === '') {
            throw MediaGenerationServiceException::llmContractFailed(
                'Media interpretation service is not configured.',
                ['config' => 'services.media_generation.llm_adapter.base_url']
            );
        }

        $request = $this->http()
            ->baseUrl($baseUrl)
            ->acceptJson()
            ->timeout($this->timeoutSeconds())
            ->connectTimeout($this->connectTimeoutSeconds())
            ->withHeaders($signedRequest['headers'])
            ->retry(
                $this->retryAttempts(),
                $this->retrySleepMilliseconds(),
                function (Exception $exception): bool {
                    return $exception instanceof ConnectionException;
                },
                false,
            );

        try {
            return $request->send('POST', $this->path(), ['body' => $signedRequest['encoded_payload']]);
        } catch (ConnectionException $exception) {
            throw MediaGenerationServiceException::llmContractFailed(
                'Could not reach the media interpretation service.',
                ['exception' => $exception->getMessage()]
            );
        }
    }

    /**
     * @return array{payload: array<string, mixed>, taxonomy_inference: array<string, mixed>|null}
     */
    protected function buildRequestContext(MediaGeneration $generation): array
    {
        $taxonomyInference = $this->resolveTaxonomyInference($generation);

        return [
            'payload' => MediaPromptInterpretationRequestContract::fromGeneration(
                $generation,
                $this->model(),
                MediaPromptInterpretationSchema::llmInstruction($taxonomyInference),
            ),
            'taxonomy_inference' => $taxonomyInference,
        ];
    }

    /**
     * @return array{payload: array<string, mixed>, used_fallback: bool, fallback_error: array<string, mixed>|null}
     */
    protected function normalizeInterpretationPayload(MediaGeneration $generation, string $rawContent, ?array $taxonomyInference = null): array
    {
        try {
            $payload = MediaPromptInterpretationSchema::decodeAndValidate($rawContent);

            return [
                'payload' => $this->enrichInterpretationPayload($generation, $payload, $taxonomyInference),
                'used_fallback' => false,
                'fallback_error' => null,
            ];
        } catch (MediaGenerationContractException $exception) {
            return [
                'payload' => MediaPromptInterpretationSchema::fallback(
                    teacherPrompt: (string) $generation->raw_prompt,
                    reasonCode: $exception->errorCode(),
                    preferredOutputType: $generation->preferred_output_type,
                    subjectContext: $this->subjectContext($generation, $taxonomyInference),
                    subSubjectContext: $this->subSubjectContext($generation, $taxonomyInference),
                    taxonomyHint: $taxonomyInference,
                ),
                'used_fallback' => true,
                'fallback_error' => [
                    'message' => $exception->getMessage(),
                    'error_code' => $exception->errorCode(),
                    'context' => $exception->context(),
                ],
            ];
        }
    }

    protected function buildAuditPayload(
        MediaGeneration $generation,
        array $providerMetadata,
        array $requestPayload,
        array $requestMeta,
        Response $response,
        string $rawContent,
        array $normalizedPayload,
        bool $usedFallback,
        ?array $fallbackError,
        ?array $taxonomyInference,
    ): array {
        return [
            'schema_version' => self::AUDIT_SCHEMA_VERSION,
            'provider' => [
                'name' => $providerMetadata['provider'],
                'model' => $providerMetadata['model'],
                'primary_provider' => $providerMetadata['primary_provider'],
                'fallback_used' => $providerMetadata['fallback_used'],
                'fallback_reason' => $providerMetadata['fallback_reason'],
                'reported_by_adapter' => $providerMetadata['reported_by_adapter'],
            ],
            'request' => $requestPayload,
            'request_meta' => $requestMeta,
            'taxonomy_inference' => $taxonomyInference,
            'response' => [
                'http_status' => $response->status(),
                'raw_payload' => $this->decodedResponsePayload($response),
                'raw_content' => $rawContent,
                'normalized_payload' => $normalizedPayload,
                'used_fallback' => $usedFallback,
                'fallback_error' => $fallbackError,
            ],
            'recorded_at' => now()->toISOString(),
        ];
    }

    protected function extractRawInterpretationContent(Response $response): string
    {
        $decodedPayload = $this->decodedResponsePayload($response);

        if (is_array($decodedPayload)) {
            foreach ([
                'output_text',
                'data.output_text',
                'data.response_text',
                'data.content',
                'response',
                'message.content',
                'choices.0.message.content',
                'choices.0.text',
                'content',
            ] as $path) {
                $content = $this->stringifyContent(data_get($decodedPayload, $path));

                if ($content !== null) {
                    return $content;
                }
            }

            if ($this->looksLikeInterpretationPayload($decodedPayload)) {
                return $this->encodeJson($decodedPayload);
            }
        }

        return trim($response->body());
    }

    /**
     * @return array<string, string|null>|null
     */
    protected function subjectContext(MediaGeneration $generation, ?array $taxonomyInference = null): ?array
    {
        $subject = $generation->subject;

        if ($subject === null || trim((string) $subject->name) === '') {
            $subjectName = trim((string) data_get($taxonomyInference, 'best_match.subject_name', ''));

            if ($subjectName === '') {
                return null;
            }

            $subjectSlug = trim((string) data_get($taxonomyInference, 'best_match.subject_slug', ''));

            return [
                'subject_name' => $subjectName,
                'subject_slug' => $subjectSlug !== '' ? $subjectSlug : null,
            ];
        }

        return [
            'subject_name' => trim((string) $subject->name),
            'subject_slug' => trim((string) $subject->slug) !== '' ? trim((string) $subject->slug) : null,
        ];
    }

    /**
     * @return array<string, string|null>|null
     */
    protected function subSubjectContext(MediaGeneration $generation, ?array $taxonomyInference = null): ?array
    {
        $subSubject = $generation->subSubject;

        if ($subSubject === null || trim((string) $subSubject->name) === '') {
            $subSubjectName = trim((string) data_get($taxonomyInference, 'best_match.sub_subject_name', ''));

            if ($subSubjectName === '') {
                return null;
            }

            $subSubjectSlug = trim((string) data_get($taxonomyInference, 'best_match.sub_subject_slug', ''));

            return [
                'sub_subject_name' => $subSubjectName,
                'sub_subject_slug' => $subSubjectSlug !== '' ? $subSubjectSlug : null,
            ];
        }

        return [
            'sub_subject_name' => trim((string) $subSubject->name),
            'sub_subject_slug' => trim((string) $subSubject->slug) !== '' ? trim((string) $subSubject->slug) : null,
        ];
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    protected function enrichInterpretationPayload(MediaGeneration $generation, array $payload, ?array $taxonomyInference = null): array
    {
        $resolvedSubjectContext = $this->subjectContext($generation, $taxonomyInference);
        $resolvedSubSubjectContext = $this->subSubjectContext($generation, $taxonomyInference);
        $enrichedPayload = $payload;

        if (($enrichedPayload['subject_context'] ?? null) === null && $resolvedSubjectContext !== null) {
            $enrichedPayload['subject_context'] = $resolvedSubjectContext;
        }

        if (($enrichedPayload['sub_subject_context'] ?? null) === null && $resolvedSubSubjectContext !== null) {
            $enrichedPayload['sub_subject_context'] = $resolvedSubSubjectContext;
        }

        if ($enrichedPayload === $payload) {
            return $payload;
        }

        return MediaPromptInterpretationSchema::validate($enrichedPayload);
    }

    /**
     * @return array<string, mixed>|null
     */
    protected function resolveTaxonomyInference(MediaGeneration $generation): ?array
    {
        $generation->loadMissing(['subject', 'subSubject.subject']);

        if ($generation->subject !== null && $generation->subSubject !== null) {
            return null;
        }

        return $this->taxonomyInferenceService()->infer((string) $generation->raw_prompt);
    }

    protected function taxonomyInferenceService(): MediaPromptTaxonomyInferenceService
    {
        return $this->taxonomyInferenceService ??= app(MediaPromptTaxonomyInferenceService::class);
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array{request_id: string, timestamp: string, signature_algorithm: string, body_sha256: string, signature: string, encoded_payload: string, headers: array<string, string>}
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
                'Media interpretation request could not be serialized for the LLM adapter.',
                ['exception' => $exception->getMessage()]
            );
        }

        return $this->requestSigner()->build($sharedSecret, (string) $generation->id, $encodedPayload);
    }

    /**
     * @param  array{request_id: string, timestamp: string, signature_algorithm: string, body_sha256: string}  $signedRequest
     * @return array<string, mixed>
     */
    protected function buildRequestMeta(array $signedRequest): array
    {
        return [
            'request_id' => $signedRequest['request_id'],
            'path' => '/' . $this->path(),
            'timestamp' => $signedRequest['timestamp'],
            'signature_algorithm' => $signedRequest['signature_algorithm'],
            'body_sha256' => $signedRequest['body_sha256'],
            'adapter' => [
                'base_url' => trim((string) config('services.media_generation.llm_adapter.base_url')),
                'service_name' => trim((string) config('services.media_generation.llm_adapter.service_name')),
                'service_version' => trim((string) config('services.media_generation.llm_adapter.service_version')),
                'request_max_age_seconds' => (int) config('services.media_generation.llm_adapter.request_max_age_seconds', 300),
                'clock_skew_seconds' => (int) config('services.media_generation.llm_adapter.clock_skew_seconds', 30),
            ],
        ];
    }

    protected function stringifyContent(mixed $value): ?string
    {
        if (is_string($value)) {
            $trimmed = trim($value);

            return $trimmed !== '' ? $trimmed : null;
        }

        if (! is_array($value)) {
            return null;
        }

        if ($this->looksLikeInterpretationPayload($value)) {
            return $this->encodeJson($value);
        }

        $segments = [];

        foreach ($value as $item) {
            if (is_array($item)) {
                $segment = $this->stringifyContent($item['text'] ?? $item['content'] ?? null);
            } else {
                $segment = $this->stringifyContent($item);
            }

            if ($segment !== null) {
                $segments[] = $segment;
            }
        }

        if ($segments === []) {
            return null;
        }

        return trim(implode("\n", $segments));
    }

    protected function looksLikeInterpretationPayload(array $payload): bool
    {
        return array_key_exists('schema_version', $payload)
            || (array_key_exists('teacher_prompt', $payload) && array_key_exists('document_blueprint', $payload));
    }

    protected function throwFailedInterpretationRequest(Response $response): never
    {
        throw MediaGenerationServiceException::llmContractFailed(
            'Media interpretation service rejected the request.',
            [
                'http_status' => $response->status(),
                'response_body' => trim($response->body()),
            ]
        );
    }

    protected function decodedResponsePayload(Response $response): mixed
    {
        $json = $response->json();

        return $json ?? trim($response->body());
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

    protected function requestSigner(): InterServiceRequestSigner
    {
        return $this->requestSigner ?? app(InterServiceRequestSigner::class);
    }

    protected function provider(): string
    {
        return trim((string) config('services.media_generation.interpreter.provider', 'llm-adapter'));
    }

    protected function model(): string
    {
        return trim((string) config('services.media_generation.interpreter.model', 'adapter-managed'));
    }

    protected function path(): string
    {
        return ltrim((string) config('services.media_generation.interpreter.path', '/v1/interpret'), '/');
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