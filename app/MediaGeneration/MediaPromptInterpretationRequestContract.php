<?php

namespace App\MediaGeneration;

use App\Models\MediaGeneration;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;

final class MediaPromptInterpretationRequestContract
{
    public const REQUEST_TYPE = 'media_prompt_interpretation';

    public static function fromGeneration(MediaGeneration $generation, string $model, string $instruction): array
    {
        $generation->loadMissing(['subject', 'subSubject.subject']);

        $subject = $generation->subSubject?->subject ?? $generation->subject;
        $subSubject = $generation->subSubject;

        return self::validate([
            'request_type' => self::REQUEST_TYPE,
            'generation_id' => (string) $generation->id,
            'model' => $model,
            'instruction' => $instruction,
            'input' => [
                'teacher_prompt' => (string) $generation->raw_prompt,
                'preferred_output_type' => MediaGeneration::normalizePreferredOutputType($generation->preferred_output_type),
                'subject_context' => $subject ? [
                    'id' => $subject->id,
                    'name' => $subject->name,
                    'slug' => $subject->slug,
                ] : null,
                'sub_subject_context' => $subSubject ? [
                    'id' => $subSubject->id,
                    'name' => $subSubject->name,
                    'slug' => $subSubject->slug,
                ] : null,
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
            'input.teacher_prompt' => ['required', 'string', 'max:5000'],
            'input.preferred_output_type' => ['required', 'string', Rule::in(MediaPromptInterpretationSchema::allowedPreferredOutputTypes())],
            'input.subject_context' => ['nullable', 'array'],
            'input.subject_context.id' => ['required_with:input.subject_context', 'integer'],
            'input.subject_context.name' => ['required_with:input.subject_context', 'string', 'max:100'],
            'input.subject_context.slug' => ['nullable', 'string', 'max:100'],
            'input.sub_subject_context' => ['nullable', 'array'],
            'input.sub_subject_context.id' => ['required_with:input.sub_subject_context', 'integer'],
            'input.sub_subject_context.name' => ['required_with:input.sub_subject_context', 'string', 'max:100'],
            'input.sub_subject_context.slug' => ['nullable', 'string', 'max:100'],
        ]);

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Prompt interpretation request payload failed validation.',
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
                'teacher_prompt' => trim($payload['input']['teacher_prompt']),
                'preferred_output_type' => MediaGeneration::normalizePreferredOutputType($payload['input']['preferred_output_type']),
                'subject_context' => self::normalizeNullableContext($payload['input']['subject_context']),
                'sub_subject_context' => self::normalizeNullableContext($payload['input']['sub_subject_context']),
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
            ['teacher_prompt', 'preferred_output_type', 'subject_context', 'sub_subject_context'],
            'input'
        );

        foreach (['subject_context', 'sub_subject_context'] as $path) {
            $context = $payload['input'][$path] ?? null;

            if (! is_array($context)) {
                continue;
            }

            self::assertAllowedKeys($context, ['id', 'name', 'slug'], 'input.' . $path);
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
            'Prompt interpretation request payload contains unsupported fields.',
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
            'subject_context' => null,
            'sub_subject_context' => null,
        ], $payload['input']);

        return $payload;
    }

    /**
     * @param  array<string, mixed>|null  $context
     * @return array<string, mixed>|null
     */
    private static function normalizeNullableContext(?array $context): ?array
    {
        if (! is_array($context)) {
            return null;
        }

        return [
            'id' => (int) $context['id'],
            'name' => trim($context['name']),
            'slug' => $context['slug'] !== null ? trim((string) $context['slug']) : null,
        ];
    }
}