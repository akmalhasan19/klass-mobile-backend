<?php

namespace App\Services;

use App\MediaGeneration\MediaDeliveryRequestContract;
use App\MediaGeneration\MediaDeliveryResponseSchema;
use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationServiceException;
use App\MediaGeneration\MediaPromptInterpretationRequestContract;
use App\MediaGeneration\MediaPromptInterpretationSchema;
use App\Services\Concerns\InteractsWithLlmAdapter;
use Exception;
use Illuminate\Http\Client\ConnectionException;
use Illuminate\Http\Client\Factory as HttpFactory;
use Illuminate\Http\Client\Response;
use Illuminate\Support\Str;
use JsonException;

class LlmAdapterSmokeTestService
{
    use InteractsWithLlmAdapter;

    public function __construct(
        protected ?HttpFactory $http = null,
        protected ?InterServiceRequestSigner $requestSigner = null,
    ) {
    }

    /**
     * @return array{
     *     interpret: array{path: string, request_id: string, provider: string, model: string},
     *     respond: array{path: string, request_id: string, provider: string, model: string}
     * }
     */
    public function exerciseRoutes(?string $expectedProvider = null): array
    {
        $sharedSecret = trim((string) config('services.media_generation.llm_adapter.shared_secret'));

        if ($sharedSecret === '') {
            throw MediaGenerationServiceException::llmContractFailed(
                'LLM adapter shared secret is not configured.',
                ['config' => 'services.media_generation.llm_adapter.shared_secret']
            );
        }

        $normalizedExpectedProvider = is_string($expectedProvider) && trim($expectedProvider) !== ''
            ? strtolower(trim($expectedProvider))
            : null;

        return [
            'interpret' => $this->exerciseInterpretationRoute($sharedSecret, $normalizedExpectedProvider),
            'respond' => $this->exerciseDeliveryRoute($sharedSecret, $normalizedExpectedProvider),
        ];
    }

    /**
     * @return array{path: string, request_id: string, provider: string, model: string}
     */
    protected function exerciseInterpretationRoute(string $sharedSecret, ?string $expectedProvider): array
    {
        $generationId = 'smoke-interpret-' . Str::lower((string) Str::ulid());
        $payload = MediaPromptInterpretationRequestContract::validate([
            'request_type' => MediaPromptInterpretationRequestContract::REQUEST_TYPE,
            'generation_id' => $generationId,
            'model' => $this->routeModel('interpreter'),
            'instruction' => MediaPromptInterpretationSchema::llmInstruction(),
            'input' => [
                'teacher_prompt' => 'Buatkan handout pecahan untuk kelas 5 dengan contoh dan latihan singkat.',
                'preferred_output_type' => 'pdf',
                'subject_context' => [
                    'id' => 10,
                    'name' => 'Matematika',
                    'slug' => 'matematika',
                ],
                'sub_subject_context' => [
                    'id' => 11,
                    'name' => 'Pecahan',
                    'slug' => 'pecahan',
                ],
            ],
        ]);

        $request = $this->sendSignedRequest('interpreter', $sharedSecret, $generationId, $payload);
        $response = $request['response'];

        if ($response->failed()) {
            throw MediaGenerationServiceException::llmContractFailed(
                'LLM adapter interpretation smoke request failed.',
                [
                    'path' => '/' . $this->routePath('interpreter'),
                    'http_status' => $response->status(),
                    'response_body' => trim($response->body()),
                ]
            );
        }

        $responsePayload = $response->json();

        if (is_array($responsePayload) && ! array_is_list($responsePayload)) {
            $responseErrorCode = data_get($responsePayload, 'error.code');

            if (is_string($responseErrorCode) && trim($responseErrorCode) !== '') {
                throw new MediaGenerationContractException(
                    'LLM adapter interpretation smoke request returned a contract-validation error envelope.',
                    'media_generation_contract_invalid',
                    [
                        'path' => '/' . $this->routePath('interpreter'),
                        'error' => data_get($responsePayload, 'error'),
                    ]
                );
            }
        }

        $validatedPayload = MediaPromptInterpretationSchema::decodeAndValidate(
            $this->extractInterpretationContent($response)
        );

        if ((bool) data_get($validatedPayload, 'fallback.triggered')) {
            throw new MediaGenerationContractException(
                'LLM adapter interpretation smoke request triggered a fallback payload.',
                'media_generation_contract_invalid',
                ['path' => '/' . $this->routePath('interpreter')]
            );
        }

        $metadata = $this->resolveLlmAdapterResponseMetadata(
            $response,
            $this->routeProvider('interpreter'),
            $this->routeModel('interpreter'),
        );

        $this->assertExpectedProvider($metadata['provider'], $expectedProvider, 'interpret');

        return [
            'path' => '/' . $this->routePath('interpreter'),
            'request_id' => $request['signed_request']['request_id'],
            'provider' => $metadata['provider'],
            'model' => $metadata['model'],
        ];
    }

    /**
     * @return array{path: string, request_id: string, provider: string, model: string}
     */
    protected function exerciseDeliveryRoute(string $sharedSecret, ?string $expectedProvider): array
    {
        $generationId = 'smoke-respond-' . Str::lower((string) Str::ulid());
        $payload = MediaDeliveryRequestContract::validate([
            'request_type' => MediaDeliveryRequestContract::REQUEST_TYPE,
            'generation_id' => $generationId,
            'model' => $this->routeModel('delivery'),
            'instruction' => MediaDeliveryResponseSchema::llmInstruction(),
            'input' => [
                'artifact' => [
                    'output_type' => 'pdf',
                    'title' => 'Handout Pecahan Kelas 5',
                    'file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                    'thumbnail_url' => 'https://example.com/gallery/handout-pecahan-kelas-5.png',
                    'mime_type' => 'application/pdf',
                    'filename' => 'handout-pecahan-kelas-5.pdf',
                ],
                'publication' => [
                    'topic' => [
                        'id' => 'topic-smoke-1',
                        'title' => 'Handout Pecahan Kelas 5',
                    ],
                    'content' => [
                        'id' => 'content-smoke-1',
                        'title' => 'Handout Pecahan Kelas 5',
                        'type' => 'brief',
                        'media_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                    ],
                    'recommended_project' => [
                        'id' => 'project-smoke-1',
                        'title' => 'Handout Pecahan Kelas 5',
                        'project_file_url' => 'https://example.com/materials/handout-pecahan-kelas-5.pdf',
                    ],
                ],
                'preview_summary' => 'Media siap digunakan untuk penguatan konsep dan latihan singkat.',
                'teacher_delivery_summary' => 'Gunakan handout ini sebagai pengantar sebelum latihan mandiri.',
                'generation_summary' => 'Handout printable untuk materi pecahan dasar.',
            ],
        ]);

        $request = $this->sendSignedRequest('delivery', $sharedSecret, $generationId, $payload);
        $response = $request['response'];

        if ($response->failed()) {
            throw MediaGenerationServiceException::llmContractFailed(
                'LLM adapter delivery smoke request failed.',
                [
                    'path' => '/' . $this->routePath('delivery'),
                    'http_status' => $response->status(),
                    'response_body' => trim($response->body()),
                ]
            );
        }

        $responsePayload = $response->json();

        if (! is_array($responsePayload) || array_is_list($responsePayload)) {
            throw new MediaGenerationContractException(
                'LLM adapter delivery smoke response must be a JSON object.',
                'media_generation_contract_invalid',
                ['path' => '/' . $this->routePath('delivery')]
            );
        }

        $validatedPayload = MediaDeliveryResponseSchema::validate($responsePayload);

        if ((bool) data_get($validatedPayload, 'fallback.triggered')) {
            throw new MediaGenerationContractException(
                'LLM adapter delivery smoke request triggered a fallback payload.',
                'media_generation_contract_invalid',
                ['path' => '/' . $this->routePath('delivery')]
            );
        }

        if ((bool) data_get($validatedPayload, 'response_meta.llm_used') !== true) {
            throw new MediaGenerationContractException(
                'LLM adapter delivery smoke response must report llm_used=true.',
                'media_generation_contract_invalid',
                ['path' => '/' . $this->routePath('delivery')]
            );
        }

        $metadata = $this->resolveLlmAdapterResponseMetadata(
            $response,
            $this->routeProvider('delivery'),
            $this->routeModel('delivery'),
        );

        $this->assertExpectedProvider($metadata['provider'], $expectedProvider, 'respond');

        return [
            'path' => '/' . $this->routePath('delivery'),
            'request_id' => $request['signed_request']['request_id'],
            'provider' => $metadata['provider'],
            'model' => $metadata['model'],
        ];
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array{
     *     response: Response,
     *     signed_request: array{request_id: string, timestamp: string, signature_algorithm: string, body_sha256: string, signature: string, encoded_payload: string, headers: array<string, string>}
     * }
     */
    protected function sendSignedRequest(string $routeKey, string $sharedSecret, string $generationId, array $payload): array
    {
        $baseUrl = $this->llmAdapterBaseUrl($routeKey);

        if ($baseUrl === '') {
            throw MediaGenerationServiceException::llmContractFailed(
                'LLM adapter service is not configured.',
                ['config' => 'services.media_generation.llm_adapter.base_url']
            );
        }

        try {
            $encodedPayload = $this->requestSigner()->encodePayload($payload);
        } catch (JsonException $exception) {
            throw MediaGenerationServiceException::llmContractFailed(
                'LLM adapter smoke payload could not be serialized.',
                ['exception' => $exception->getMessage()]
            );
        }

        $signedRequest = $this->requestSigner()->build($sharedSecret, $generationId, $encodedPayload);

        $request = $this->http()
            ->baseUrl($baseUrl)
            ->acceptJson()
            ->timeout($this->timeoutSeconds($routeKey))
            ->connectTimeout($this->connectTimeoutSeconds($routeKey))
            ->withHeaders($signedRequest['headers'])
            ->retry(
                $this->retryAttempts($routeKey),
                $this->retrySleepMilliseconds($routeKey),
                function (Exception $exception): bool {
                    return $exception instanceof ConnectionException;
                },
                false,
            );

        try {
            $response = $request->send('POST', $this->routePath($routeKey), ['body' => $signedRequest['encoded_payload']]);
        } catch (ConnectionException $exception) {
            throw MediaGenerationServiceException::llmContractFailed(
                'Could not reach the LLM adapter smoke route.',
                [
                    'path' => '/' . $this->routePath($routeKey),
                    'exception' => $exception->getMessage(),
                ]
            );
        }

        return [
            'response' => $response,
            'signed_request' => $signedRequest,
        ];
    }

    protected function extractInterpretationContent(Response $response): string
    {
        $decodedPayload = $response->json();

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
                $content = $this->stringifyInterpretationContent(data_get($decodedPayload, $path));

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

    protected function stringifyInterpretationContent(mixed $value): ?string
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
                $segment = $this->stringifyInterpretationContent($item['text'] ?? $item['content'] ?? null);
            } else {
                $segment = $this->stringifyInterpretationContent($item);
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
            && array_key_exists('teacher_prompt', $payload)
            && array_key_exists('teacher_intent', $payload)
            && array_key_exists('document_blueprint', $payload);
    }

    protected function encodeJson(array $payload): string
    {
        try {
            return json_encode($payload, JSON_THROW_ON_ERROR | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
        } catch (JsonException) {
            return '{}';
        }
    }

    protected function assertExpectedProvider(string $actualProvider, ?string $expectedProvider, string $route): void
    {
        if ($expectedProvider === null) {
            return;
        }

        if (strtolower(trim($actualProvider)) !== $expectedProvider) {
            throw new MediaGenerationContractException(
                'LLM adapter smoke route reported an unexpected provider.',
                'media_generation_contract_invalid',
                [
                    'route' => $route,
                    'expected_provider' => $expectedProvider,
                    'actual_provider' => $actualProvider,
                ]
            );
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

    protected function routePath(string $routeKey): string
    {
        return ltrim((string) config('services.media_generation.' . $routeKey . '.path'), '/');
    }

    protected function routeProvider(string $routeKey): string
    {
        return trim((string) config('services.media_generation.' . $routeKey . '.provider', 'llm-adapter'));
    }

    protected function routeModel(string $routeKey): string
    {
        return trim((string) config('services.media_generation.' . $routeKey . '.model', 'adapter-managed'));
    }

    protected function timeoutSeconds(string $routeKey): float
    {
        return (float) config('services.media_generation.' . $routeKey . '.timeout_seconds', 30);
    }

    protected function connectTimeoutSeconds(string $routeKey): float
    {
        return (float) config('services.media_generation.' . $routeKey . '.connect_timeout_seconds', 10);
    }

    protected function retryAttempts(string $routeKey): int
    {
        return (int) config('services.media_generation.' . $routeKey . '.retry_attempts', 2);
    }

    protected function retrySleepMilliseconds(string $routeKey): int
    {
        return (int) config('services.media_generation.' . $routeKey . '.retry_sleep_milliseconds', 250);
    }
}