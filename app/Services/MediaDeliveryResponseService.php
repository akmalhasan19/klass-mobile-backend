<?php

namespace App\Services;

use App\MediaGeneration\MediaDeliveryRequestContract;
use App\MediaGeneration\MediaDeliveryResponseSchema;
use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationServiceException;
use App\Models\MediaGeneration;
use App\Services\Concerns\InteractsWithLlmAdapter;
use Exception;
use Illuminate\Http\Client\ConnectionException;
use Illuminate\Http\Client\Factory as HttpFactory;
use Illuminate\Http\Client\Response;
use JsonException;

class MediaDeliveryResponseService
{
    use InteractsWithLlmAdapter;

    public function __construct(
        protected ?HttpFactory $http = null,
        protected ?InterServiceRequestSigner $requestSigner = null,
    )
    {
    }

    public function compose(MediaGeneration $generation): MediaGeneration
    {
        $generation = $generation->fresh(['topic', 'content', 'recommendedProject', 'subject', 'subSubject.subject']) ?? $generation;
        $context = $this->buildContext($generation);

        if (! $this->shouldCallLlm($generation)) {
            $deliveryPayload = MediaDeliveryResponseSchema::fallback($context, 'delivery_service_unconfigured');

            $generation->forceFill(['delivery_payload' => $deliveryPayload])->save();

            return $generation->fresh(['topic', 'content', 'recommendedProject', 'subject', 'subSubject.subject']);
        }

        try {
            $response = $this->sendDeliveryRequest($generation, $context);

            if ($response->failed()) {
                throw MediaGenerationServiceException::llmContractFailed(
                    'Media delivery response service rejected the request.',
                    ['http_status' => $response->status(), 'response_body' => trim($response->body())]
                );
            }

            $rawContent = $this->extractRawContent($response);
            $decoded = $this->decodeJsonPayload($rawContent);
            $adapterMetadata = $this->resolveLlmAdapterResponseMetadata(
                $response,
                $this->provider(),
                $this->model(),
            );
            $deliveryPayload = MediaDeliveryResponseSchema::validate(array_replace_recursive($decoded, [
                'response_meta' => [
                    'generated_at' => is_string(data_get($decoded, 'response_meta.generated_at'))
                        && trim((string) data_get($decoded, 'response_meta.generated_at')) !== ''
                            ? trim((string) data_get($decoded, 'response_meta.generated_at'))
                            : now()->toISOString(),
                    'llm_used' => (bool) data_get($decoded, 'response_meta.llm_used', true),
                    'provider' => $adapterMetadata['provider'],
                    'model' => $adapterMetadata['model'],
                ],
                'fallback' => [
                    'triggered' => (bool) data_get($decoded, 'fallback.triggered', false),
                    'reason_code' => data_get($decoded, 'fallback.reason_code'),
                    'action' => data_get($decoded, 'fallback.action'),
                ],
            ]));
        } catch (Exception $exception) {
            $deliveryPayload = MediaDeliveryResponseSchema::fallback(
                $context,
                $exception instanceof MediaGenerationContractException || $exception instanceof MediaGenerationServiceException
                    ? 'llm_contract_failed'
                    : 'delivery_generation_failed'
            );
        }

        $generation->forceFill(['delivery_payload' => $deliveryPayload])->save();

        return $generation->fresh(['topic', 'content', 'recommendedProject', 'subject', 'subSubject.subject']);
    }

    /**
     * @return array<string, mixed>
     */
    protected function buildContext(MediaGeneration $generation): array
    {
        $artifactMetadata = data_get($generation->generator_service_response, 'response.artifact_metadata')
            ?? data_get($generation->generator_service_response, 'artifact_metadata')
            ?? [];
        $title = $this->resolveTitle($generation);
        $previewSummary = $this->resolvePreviewSummary($generation);

        return [
            'title' => $title,
            'preview_summary' => $previewSummary,
            'teacher_message' => $this->buildFallbackTeacherMessage($generation, $previewSummary),
            'recommended_next_steps' => $this->recommendedNextSteps($generation),
            'classroom_tips' => $this->classroomTips($generation),
            'artifact' => [
                'output_type' => $generation->resolved_output_type,
                'title' => $title,
                'file_url' => $generation->file_url,
                'thumbnail_url' => $generation->thumbnail_url,
                'mime_type' => $generation->mime_type,
                'filename' => data_get($artifactMetadata, 'filename'),
            ],
            'publication' => [
                'topic' => $generation->topic ? [
                    'id' => (string) $generation->topic->getKey(),
                    'title' => $generation->topic->title,
                ] : null,
                'content' => $generation->content ? [
                    'id' => (string) $generation->content->getKey(),
                    'title' => $generation->content->title,
                    'type' => $generation->content->type,
                    'media_url' => $generation->content->media_url,
                ] : null,
                'recommended_project' => $generation->recommendedProject ? [
                    'id' => (string) $generation->recommendedProject->getKey(),
                    'title' => $generation->recommendedProject->title,
                    'project_file_url' => $generation->recommendedProject->project_file_url,
                ] : null,
            ],
        ];
    }

    protected function shouldCallLlm(MediaGeneration $generation): bool
    {
        return is_string($generation->file_url)
            && trim($generation->file_url) !== ''
            && $this->llmAdapterConfigured('delivery');
    }

    /**
     * @param  array<string, mixed>  $context
     */
    protected function sendDeliveryRequest(MediaGeneration $generation, array $context): Response
    {
        $baseUrl = $this->llmAdapterBaseUrl('delivery');

        if ($baseUrl === '') {
            throw MediaGenerationServiceException::llmContractFailed(
                'Media delivery response service is not configured.',
                ['config' => 'services.media_generation.llm_adapter.base_url']
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

        $payload = MediaDeliveryRequestContract::fromGeneration(
            $generation,
            $context,
            $this->model(),
            MediaDeliveryResponseSchema::llmInstruction(),
        );
        $signedRequest = $this->buildSignedRequestContext($generation, $payload);
        $request = $request->withHeaders($signedRequest['headers']);

        try {
            return $request->send('POST', $this->path(), ['body' => $signedRequest['encoded_payload']]);
        } catch (ConnectionException $exception) {
            throw MediaGenerationServiceException::llmContractFailed(
                'Could not reach the media delivery response service.',
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
     * @return array<string, mixed>
     */
    protected function decodeJsonPayload(string $rawContent): array
    {
        try {
            $decoded = json_decode($rawContent, true, 512, JSON_THROW_ON_ERROR);
        } catch (JsonException $exception) {
            throw new MediaGenerationContractException(
                'Delivery response returned invalid JSON.',
                'llm_contract_failed',
                ['json_error' => $exception->getMessage()]
            );
        }

        if (! is_array($decoded) || array_is_list($decoded)) {
            throw new MediaGenerationContractException(
                'Delivery response must be a JSON object.',
                'llm_contract_failed'
            );
        }

        return $decoded;
    }

    protected function resolveTitle(MediaGeneration $generation): string
    {
        foreach ([
            data_get($generation->generation_spec_payload, 'title'),
            data_get($generation->interpretation_payload, 'document_blueprint.title'),
            $generation->content?->title,
            $generation->topic?->title,
        ] as $candidate) {
            if (is_string($candidate) && trim($candidate) !== '') {
                return trim($candidate);
            }
        }

        return 'Media pembelajaran siap digunakan';
    }

    protected function resolvePreviewSummary(MediaGeneration $generation): string
    {
        foreach ([
            data_get($generation->generation_spec_payload, 'teacher_delivery_summary'),
            data_get($generation->interpretation_payload, 'teacher_delivery_summary'),
            data_get($generation->generation_spec_payload, 'summary'),
            $generation->recommendedProject?->description,
        ] as $candidate) {
            if (is_string($candidate) && trim($candidate) !== '') {
                return trim($candidate);
            }
        }

        return 'Media berhasil dibuat dan dipublikasikan untuk digunakan di kelas atau dibagikan ke siswa.';
    }

    protected function buildFallbackTeacherMessage(MediaGeneration $generation, string $previewSummary): string
    {
        $format = strtoupper((string) $generation->resolved_output_type);

        return trim($previewSummary . ' Format akhir: ' . $format . '. File dapat dibuka dari kartu hasil ini atau melalui workspace Anda.');
    }

    /**
     * @return string[]
     */
    protected function recommendedNextSteps(MediaGeneration $generation): array
    {
        $steps = [
            'Tinjau struktur materi sebelum dibagikan ke siswa.',
            'Gunakan file ini sebagai materi utama atau pelengkap pembelajaran.',
        ];

        if ($generation->recommendedProject !== null) {
            $steps[] = 'Buka hasil publikasi di homepage recommendation feed untuk memastikan tampilannya sesuai.';
        }

        return $steps;
    }

    /**
     * @return string[]
     */
    protected function classroomTips(MediaGeneration $generation): array
    {
        $tips = ['Mulai dari ringkasan utama sebelum masuk ke bagian latihan atau aktivitas.'];

        if ($generation->resolved_output_type === 'pptx') {
            $tips[] = 'Gunakan satu slide awal untuk menjelaskan tujuan belajar sebelum diskusi kelas dimulai.';
        } else {
            $tips[] = 'Ajak siswa menandai bagian penting pada dokumen sebelum pengerjaan tugas mandiri.';
        }

        return $tips;
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
                'Media delivery request could not be serialized for the LLM adapter.',
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
        return trim((string) config('services.media_generation.delivery.provider', 'llm-adapter'));
    }

    protected function model(): string
    {
        return trim((string) config('services.media_generation.delivery.model', 'adapter-managed'));
    }

    protected function path(): string
    {
        return ltrim((string) config('services.media_generation.delivery.path', '/v1/respond'), '/');
    }

    protected function timeoutSeconds(): float
    {
        return (float) config('services.media_generation.delivery.timeout_seconds', 30);
    }

    protected function connectTimeoutSeconds(): float
    {
        return (float) config('services.media_generation.delivery.connect_timeout_seconds', 10);
    }

    protected function retryAttempts(): int
    {
        return (int) config('services.media_generation.delivery.retry_attempts', 2);
    }

    protected function retrySleepMilliseconds(): int
    {
        return (int) config('services.media_generation.delivery.retry_sleep_milliseconds', 250);
    }
}