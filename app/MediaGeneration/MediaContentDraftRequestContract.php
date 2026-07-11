<?php

namespace App\MediaGeneration;

use App\Models\MediaGeneration;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;

final class MediaContentDraftRequestContract
{
    public const REQUEST_TYPE = 'media_content_draft';

    /**
     * @param  array<string, mixed>  $decision
     */
    public static function fromGeneration(MediaGeneration $generation, array $decision, string $model, string $instruction): array
    {
        return self::validate([
            'request_type' => self::REQUEST_TYPE,
            'generation_id' => (string) $generation->id,
            'model' => $model,
            'instruction' => $instruction,
            'input' => [
                'resolved_output_type' => data_get($decision, 'resolved_output_type'),
                'interpretation' => $generation->interpretation_payload,
                'taxonomy_hint' => MediaDraftTaxonomyHint::fromGeneration($generation),
            ],
        ]);
    }

    public static function validate(array $payload): array
    {
        self::assertAllowedKeys($payload, ['request_type', 'generation_id', 'model', 'instruction', 'input'], 'payload');
        self::assertNestedAllowedKeys($payload);

        $validator = Validator::make($payload, [
            'request_type' => ['required', 'string', Rule::in([self::REQUEST_TYPE])],
            'generation_id' => ['required', 'string', 'max:100'],
            'model' => ['required', 'string', 'max:200'],
            'instruction' => ['required', 'string', 'max:20000'],
            'input' => ['required', 'array'],
            'input.resolved_output_type' => ['required', 'string', Rule::in(MediaPromptInterpretationSchema::allowedOutputFormats())],
            'input.interpretation' => ['required', 'array'],
            'input.taxonomy_hint' => ['nullable', 'array'],
        ]);

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Content draft request payload failed validation.',
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
                'resolved_output_type' => trim($payload['input']['resolved_output_type']),
                'interpretation' => MediaPromptInterpretationSchema::validate((array) $payload['input']['interpretation']),
                'taxonomy_hint' => MediaDraftTaxonomyHint::validate(
                    is_array($payload['input']['taxonomy_hint'] ?? null)
                        ? $payload['input']['taxonomy_hint']
                        : null
                ),
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
            ['resolved_output_type', 'interpretation', 'taxonomy_hint'],
            'input'
        );
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
            'Content draft request payload contains unsupported fields.',
            MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
            [
                'path' => $path,
                'unknown_fields' => array_values($unknownKeys),
            ]
        );
    }
}