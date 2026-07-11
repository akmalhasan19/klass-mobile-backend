<?php

namespace App\MediaGeneration;

use App\Models\MediaGeneration;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;

final class MediaDeliveryRequestContract
{
    public const REQUEST_TYPE = 'media_delivery_response';

    /**
     * @param  array<string, mixed>  $context
     */
    public static function fromGeneration(MediaGeneration $generation, array $context, string $model, string $instruction): array
    {
        return self::validate([
            'request_type' => self::REQUEST_TYPE,
            'generation_id' => (string) $generation->id,
            'model' => $model,
            'instruction' => $instruction,
            'input' => [
                'artifact' => data_get($context, 'artifact', []),
                'publication' => data_get($context, 'publication', []),
                'preview_summary' => data_get($context, 'preview_summary'),
                'teacher_delivery_summary' => data_get($generation->generation_spec_payload, 'teacher_delivery_summary')
                    ?? data_get($generation->interpretation_payload, 'teacher_delivery_summary'),
                'generation_summary' => data_get($generation->generation_spec_payload, 'summary'),
            ],
        ]);
    }

    public static function validate(array $payload): array
    {
        self::assertAllowedKeys($payload, ['request_type', 'generation_id', 'model', 'instruction', 'input'], 'payload');
        self::assertNestedAllowedKeys($payload);

        $payload = self::applyDefaults($payload);

        $validator = Validator::make($payload, [
            'request_type' => ['required', 'string', Rule::in([self::REQUEST_TYPE])],
            'generation_id' => ['required', 'string', 'max:100'],
            'model' => ['required', 'string', 'max:200'],
            'instruction' => ['required', 'string', 'max:20000'],
            'input' => ['required', 'array'],
            'input.artifact' => ['required', 'array'],
            'input.artifact.output_type' => ['required', 'string', Rule::in(MediaPromptInterpretationSchema::allowedOutputFormats())],
            'input.artifact.title' => ['required', 'string', 'max:200'],
            'input.artifact.file_url' => ['required', 'string', 'max:2048'],
            'input.artifact.thumbnail_url' => ['nullable', 'string', 'max:2048'],
            'input.artifact.mime_type' => ['required', 'string', 'max:255'],
            'input.artifact.filename' => ['nullable', 'string', 'max:255'],
            'input.publication' => ['required', 'array'],
            'input.publication.topic' => ['nullable', 'array'],
            'input.publication.topic.id' => ['required_with:input.publication.topic', 'string', 'max:100'],
            'input.publication.topic.title' => ['required_with:input.publication.topic', 'string', 'max:200'],
            'input.publication.content' => ['nullable', 'array'],
            'input.publication.content.id' => ['required_with:input.publication.content', 'string', 'max:100'],
            'input.publication.content.title' => ['required_with:input.publication.content', 'string', 'max:200'],
            'input.publication.content.type' => ['nullable', 'string', 'max:100'],
            'input.publication.content.media_url' => ['nullable', 'string', 'max:2048'],
            'input.publication.recommended_project' => ['nullable', 'array'],
            'input.publication.recommended_project.id' => ['required_with:input.publication.recommended_project', 'string', 'max:100'],
            'input.publication.recommended_project.title' => ['required_with:input.publication.recommended_project', 'string', 'max:200'],
            'input.publication.recommended_project.project_file_url' => ['nullable', 'string', 'max:2048'],
            'input.preview_summary' => ['required', 'string', 'max:1000'],
            'input.teacher_delivery_summary' => ['nullable', 'string', 'max:1000'],
            'input.generation_summary' => ['nullable', 'string', 'max:2000'],
        ]);

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Delivery request payload failed validation.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                ['errors' => $validator->errors()->toArray()]
            );
        }

        return [
            'request_type' => trim($payload['request_type']),
            'generation_id' => trim($payload['generation_id']),
            'model' => trim($payload['model']),
            'instruction' => trim($payload['instruction']),
            'input' => [
                'artifact' => [
                    'output_type' => trim($payload['input']['artifact']['output_type']),
                    'title' => trim($payload['input']['artifact']['title']),
                    'file_url' => trim($payload['input']['artifact']['file_url']),
                    'thumbnail_url' => $payload['input']['artifact']['thumbnail_url'] !== null
                        ? trim((string) $payload['input']['artifact']['thumbnail_url'])
                        : null,
                    'mime_type' => trim($payload['input']['artifact']['mime_type']),
                    'filename' => $payload['input']['artifact']['filename'] !== null
                        ? trim((string) $payload['input']['artifact']['filename'])
                        : null,
                ],
                'publication' => [
                    'topic' => self::normalizeNullableNode($payload['input']['publication']['topic']),
                    'content' => self::normalizeNullableNode($payload['input']['publication']['content']),
                    'recommended_project' => self::normalizeNullableNode($payload['input']['publication']['recommended_project']),
                ],
                'preview_summary' => trim($payload['input']['preview_summary']),
                'teacher_delivery_summary' => $payload['input']['teacher_delivery_summary'] !== null
                    ? trim((string) $payload['input']['teacher_delivery_summary'])
                    : null,
                'generation_summary' => $payload['input']['generation_summary'] !== null
                    ? trim((string) $payload['input']['generation_summary'])
                    : null,
            ],
        ];
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private static function assertNestedAllowedKeys(array $payload): void
    {
        if (! isset($payload['input']) || ! is_array($payload['input'])) {
            return;
        }

        self::assertAllowedKeys(
            $payload['input'],
            ['artifact', 'publication', 'preview_summary', 'teacher_delivery_summary', 'generation_summary'],
            'input'
        );

        if (isset($payload['input']['artifact']) && is_array($payload['input']['artifact'])) {
            self::assertAllowedKeys(
                $payload['input']['artifact'],
                ['output_type', 'title', 'file_url', 'thumbnail_url', 'mime_type', 'filename'],
                'input.artifact'
            );
        }

        if (isset($payload['input']['publication']) && is_array($payload['input']['publication'])) {
            self::assertAllowedKeys(
                $payload['input']['publication'],
                ['topic', 'content', 'recommended_project'],
                'input.publication'
            );
        }

        foreach (['topic', 'content', 'recommended_project'] as $node) {
            $nodePayload = data_get($payload, 'input.publication.' . $node);

            if (! is_array($nodePayload)) {
                continue;
            }

            $allowedKeys = match ($node) {
                'topic' => ['id', 'title'],
                'content' => ['id', 'title', 'type', 'media_url'],
                'recommended_project' => ['id', 'title', 'project_file_url'],
            };

            self::assertAllowedKeys($nodePayload, $allowedKeys, 'input.publication.' . $node);
        }
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
            'Delivery request payload contains unsupported fields.',
            MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
            [
                'path' => $path,
                'unknown_fields' => array_values($unknownKeys),
            ]
        );
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    private static function applyDefaults(array $payload): array
    {
        if (! isset($payload['input']) || ! is_array($payload['input'])) {
            return $payload;
        }

        $payload['input'] = array_merge([
            'teacher_delivery_summary' => null,
            'generation_summary' => null,
        ], $payload['input']);

        if (isset($payload['input']['artifact']) && is_array($payload['input']['artifact'])) {
            $payload['input']['artifact'] = array_merge([
                'thumbnail_url' => null,
                'filename' => null,
            ], $payload['input']['artifact']);
        }

        if (isset($payload['input']['publication']) && is_array($payload['input']['publication'])) {
            $payload['input']['publication'] = array_merge([
                'topic' => null,
                'content' => null,
                'recommended_project' => null,
            ], $payload['input']['publication']);
        }

        return $payload;
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
}