<?php

namespace App\MediaGeneration;

use Illuminate\Support\Arr;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;

final class MediaDeliveryResponseSchema
{
    public const VERSION = 'media_delivery_response.v1';

    public static function llmInstruction(): string
    {
        return implode("\n", [
            'Create the final teacher-facing response for a completed media generation.',
            'Return exactly one JSON object.',
            'Do not wrap the JSON in markdown fences.',
            'Do not add prose before or after the JSON.',
            'Use schema_version "' . self::VERSION . '".',
            'Always include these top-level keys: schema_version, title, preview_summary, teacher_message, recommended_next_steps, classroom_tips, artifact, publication, response_meta, fallback.',
            'Recommended next steps and classroom_tips must be concise arrays of strings.',
            'Do not include any raw binary, base64, or attachment bytes.',
        ]);
    }

    public static function validate(array $payload): array
    {
        self::assertAllowedKeys($payload, self::topLevelKeys(), 'payload');
        self::assertNestedAllowedKeys($payload);

        $payload = self::applyDefaults($payload);

        $validator = Validator::make($payload, [
            'schema_version' => ['required', 'string', Rule::in([self::VERSION])],
            'title' => ['required', 'string', 'max:200'],
            'preview_summary' => ['required', 'string', 'max:1000'],
            'teacher_message' => ['required', 'string', 'max:2000'],
            'recommended_next_steps' => ['present', 'array'],
            'recommended_next_steps.*' => ['string', 'max:300'],
            'classroom_tips' => ['present', 'array'],
            'classroom_tips.*' => ['string', 'max:300'],
            'artifact' => ['required', 'array'],
            'artifact.output_type' => ['required', 'string', Rule::in(MediaPromptInterpretationSchema::allowedOutputFormats())],
            'artifact.title' => ['required', 'string', 'max:200'],
            'artifact.file_url' => ['required', 'string', 'max:2048'],
            'artifact.thumbnail_url' => ['nullable', 'string', 'max:2048'],
            'artifact.mime_type' => ['required', 'string', 'max:255'],
            'artifact.filename' => ['nullable', 'string', 'max:255'],
            'publication' => ['required', 'array'],
            'publication.topic' => ['nullable', 'array'],
            'publication.topic.id' => ['required_with:publication.topic', 'string', 'max:100'],
            'publication.topic.title' => ['required_with:publication.topic', 'string', 'max:200'],
            'publication.content' => ['nullable', 'array'],
            'publication.content.id' => ['required_with:publication.content', 'string', 'max:100'],
            'publication.content.title' => ['required_with:publication.content', 'string', 'max:200'],
            'publication.content.type' => ['nullable', 'string', 'max:100'],
            'publication.content.media_url' => ['nullable', 'string', 'max:2048'],
            'publication.recommended_project' => ['nullable', 'array'],
            'publication.recommended_project.id' => ['required_with:publication.recommended_project', 'string', 'max:100'],
            'publication.recommended_project.title' => ['required_with:publication.recommended_project', 'string', 'max:200'],
            'publication.recommended_project.project_file_url' => ['nullable', 'string', 'max:2048'],
            'response_meta' => ['required', 'array'],
            'response_meta.generated_at' => ['required', 'string', 'max:100'],
            'response_meta.llm_used' => ['required', 'boolean'],
            'response_meta.provider' => ['nullable', 'string', 'max:100'],
            'response_meta.model' => ['nullable', 'string', 'max:100'],
            'fallback' => ['required', 'array'],
            'fallback.triggered' => ['required', 'boolean'],
            'fallback.reason_code' => ['nullable', 'string', 'max:100'],
            'fallback.action' => ['nullable', 'string', 'max:100'],
        ]);

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Delivery response payload failed validation.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                ['errors' => $validator->errors()->toArray()]
            );
        }

        return self::normalize($payload);
    }

    /**
     * @param  array<string, mixed>  $context
     * @return array<string, mixed>
     */
    public static function fallback(array $context, string $reasonCode = MediaGenerationErrorCode::LLM_CONTRACT_FAILED): array
    {
        return self::validate([
            'schema_version' => self::VERSION,
            'title' => Arr::get($context, 'title', 'Media pembelajaran siap digunakan'),
            'preview_summary' => Arr::get($context, 'preview_summary', 'Media berhasil dibuat dan siap dibuka atau dibagikan.'),
            'teacher_message' => Arr::get(
                $context,
                'teacher_message',
                'Media pembelajaran berhasil dibuat. Anda dapat membuka file, membagikannya ke siswa, atau menggunakannya sebagai materi di kelas.'
            ),
            'recommended_next_steps' => Arr::wrap(Arr::get($context, 'recommended_next_steps', [
                'Tinjau file akhir sebelum dibagikan ke siswa.',
                'Gunakan materi ini sebagai pembuka atau penguatan sesi belajar.',
            ])),
            'classroom_tips' => Arr::wrap(Arr::get($context, 'classroom_tips', [
                'Gunakan ringkasan materi sebagai pengantar sebelum latihan.',
                'Sesuaikan tempo penyampaian dengan level siswa di kelas Anda.',
            ])),
            'artifact' => Arr::get($context, 'artifact', []),
            'publication' => Arr::get($context, 'publication', []),
            'response_meta' => [
                'generated_at' => now()->toISOString(),
                'llm_used' => false,
                'provider' => null,
                'model' => null,
            ],
            'fallback' => [
                'triggered' => true,
                'reason_code' => $reasonCode,
                'action' => 'use_deterministic_delivery_response',
            ],
        ]);
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    private static function normalize(array $payload): array
    {
        return [
            'schema_version' => self::VERSION,
            'title' => trim($payload['title']),
            'preview_summary' => trim($payload['preview_summary']),
            'teacher_message' => trim($payload['teacher_message']),
            'recommended_next_steps' => array_values($payload['recommended_next_steps']),
            'classroom_tips' => array_values($payload['classroom_tips']),
            'artifact' => [
                'output_type' => trim($payload['artifact']['output_type']),
                'title' => trim($payload['artifact']['title']),
                'file_url' => trim($payload['artifact']['file_url']),
                'thumbnail_url' => isset($payload['artifact']['thumbnail_url']) && $payload['artifact']['thumbnail_url'] !== null
                    ? trim($payload['artifact']['thumbnail_url'])
                    : null,
                'mime_type' => trim($payload['artifact']['mime_type']),
                'filename' => isset($payload['artifact']['filename']) && $payload['artifact']['filename'] !== null
                    ? trim($payload['artifact']['filename'])
                    : null,
            ],
            'publication' => [
                'topic' => self::normalizeNullableNode($payload['publication']['topic']),
                'content' => self::normalizeNullableNode($payload['publication']['content']),
                'recommended_project' => self::normalizeNullableNode($payload['publication']['recommended_project']),
            ],
            'response_meta' => [
                'generated_at' => trim($payload['response_meta']['generated_at']),
                'llm_used' => (bool) $payload['response_meta']['llm_used'],
                'provider' => $payload['response_meta']['provider'] !== null ? trim($payload['response_meta']['provider']) : null,
                'model' => $payload['response_meta']['model'] !== null ? trim($payload['response_meta']['model']) : null,
            ],
            'fallback' => [
                'triggered' => (bool) $payload['fallback']['triggered'],
                'reason_code' => $payload['fallback']['reason_code'] !== null ? trim($payload['fallback']['reason_code']) : null,
                'action' => $payload['fallback']['action'] !== null ? trim($payload['fallback']['action']) : null,
            ],
        ];
    }

    /**
     * @param  array<string, mixed>|null  $payload
     * @return array<string, mixed>|null
     */
    private static function normalizeNullableNode(?array $payload): ?array
    {
        if (! is_array($payload)) {
            return null;
        }

        return array_map(
            static fn (mixed $value): mixed => is_string($value) ? trim($value) : $value,
            $payload
        );
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    private static function applyDefaults(array $payload): array
    {
        return array_replace_recursive([
            'recommended_next_steps' => [],
            'classroom_tips' => [],
            'artifact' => [
                'thumbnail_url' => null,
                'filename' => null,
            ],
            'publication' => [
                'topic' => null,
                'content' => null,
                'recommended_project' => null,
            ],
            'response_meta' => [
                'generated_at' => now()->toISOString(),
                'llm_used' => false,
                'provider' => null,
                'model' => null,
            ],
            'fallback' => [
                'triggered' => false,
                'reason_code' => null,
                'action' => null,
            ],
        ], $payload);
    }

    /**
     * @param  array<string, mixed>  $payload
     * @param  string[]  $allowedKeys
     */
    private static function assertAllowedKeys(array $payload, array $allowedKeys, string $path): void
    {
        $unknownKeys = array_diff(array_keys($payload), $allowedKeys);

        if ($unknownKeys === []) {
            return;
        }

        throw new MediaGenerationContractException(
            'Delivery response payload contains unsupported fields.',
            MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
            [
                'path' => $path,
                'unknown_fields' => array_values($unknownKeys),
            ]
        );
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private static function assertNestedAllowedKeys(array $payload): void
    {
        if (isset($payload['artifact']) && is_array($payload['artifact'])) {
            self::assertAllowedKeys($payload['artifact'], ['output_type', 'title', 'file_url', 'thumbnail_url', 'mime_type', 'filename'], 'artifact');
        }

        if (isset($payload['publication']) && is_array($payload['publication'])) {
            self::assertAllowedKeys($payload['publication'], ['topic', 'content', 'recommended_project'], 'publication');
        }

        foreach (['topic', 'content', 'recommended_project'] as $node) {
            $nodePayload = data_get($payload, 'publication.' . $node);

            if (! is_array($nodePayload)) {
                continue;
            }

            $allowed = match ($node) {
                'topic' => ['id', 'title'],
                'content' => ['id', 'title', 'type', 'media_url'],
                'recommended_project' => ['id', 'title', 'project_file_url'],
            };

            self::assertAllowedKeys($nodePayload, $allowed, 'publication.' . $node);
        }

        if (isset($payload['response_meta']) && is_array($payload['response_meta'])) {
            self::assertAllowedKeys($payload['response_meta'], ['generated_at', 'llm_used', 'provider', 'model'], 'response_meta');
        }

        if (isset($payload['fallback']) && is_array($payload['fallback'])) {
            self::assertAllowedKeys($payload['fallback'], ['triggered', 'reason_code', 'action'], 'fallback');
        }
    }

    /**
     * @return string[]
     */
    private static function topLevelKeys(): array
    {
        return [
            'schema_version',
            'title',
            'preview_summary',
            'teacher_message',
            'recommended_next_steps',
            'classroom_tips',
            'artifact',
            'publication',
            'response_meta',
            'fallback',
        ];
    }
}