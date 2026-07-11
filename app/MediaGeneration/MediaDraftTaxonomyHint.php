<?php

namespace App\MediaGeneration;

use App\Models\MediaGeneration;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;

final class MediaDraftTaxonomyHint
{
    public const VERSION = 'media_draft_taxonomy_hint.v1';

    /**
     * @return array<string, mixed>|null
     */
    public static function fromGeneration(MediaGeneration $generation): ?array
    {
        $generation->loadMissing(['subject', 'subSubject.subject']);

        $persistedSubSubject = $generation->subSubject;
        $persistedSubject = $persistedSubSubject?->subject ?? $generation->subject;
        $taxonomyInference = is_array(data_get($generation->interpretation_audit_payload, 'taxonomy_inference'))
            ? data_get($generation->interpretation_audit_payload, 'taxonomy_inference')
            : null;

        $subjectName = self::firstNonEmptyString([
            $persistedSubject?->name,
            data_get($taxonomyInference, 'best_match.subject_name'),
            data_get($generation->interpretation_payload, 'subject_context.subject_name'),
        ]);

        if ($subjectName === null) {
            return null;
        }

        $subjectSlug = self::firstNonEmptyString([
            $persistedSubject?->slug,
            data_get($taxonomyInference, 'best_match.subject_slug'),
            data_get($generation->interpretation_payload, 'subject_context.subject_slug'),
        ]);

        $subSubjectName = self::firstNonEmptyString([
            $persistedSubSubject?->name,
            data_get($taxonomyInference, 'best_match.sub_subject_name'),
            data_get($generation->interpretation_payload, 'sub_subject_context.sub_subject_name'),
        ]);

        $source = match (true) {
            $persistedSubject !== null || $persistedSubSubject !== null => 'submission_context',
            is_array($taxonomyInference) => 'prompt_inference',
            self::firstNonEmptyString([
                data_get($generation->interpretation_payload, 'subject_context.subject_name'),
                data_get($generation->interpretation_payload, 'sub_subject_context.sub_subject_name'),
            ]) !== null => 'interpretation_context',
            default => null,
        };

        if ($source === null) {
            return null;
        }

        return self::validate([
            'schema_version' => self::VERSION,
            'source' => $source,
            'confidence' => [
                'score' => is_numeric(data_get($taxonomyInference, 'confidence.score'))
                    ? round((float) data_get($taxonomyInference, 'confidence.score'), 4)
                    : null,
                'label' => self::firstNonEmptyString([
                    data_get($taxonomyInference, 'confidence.label'),
                ]),
            ],
            'subject' => [
                'id' => $persistedSubject?->id ?? self::nullableInt(data_get($taxonomyInference, 'best_match.subject_id')),
                'name' => $subjectName,
                'slug' => $subjectSlug,
            ],
            'sub_subject' => $subSubjectName !== null ? [
                'id' => $persistedSubSubject?->id ?? self::nullableInt(data_get($taxonomyInference, 'best_match.sub_subject_id')),
                'subject_id' => $persistedSubSubject?->subject_id
                    ?? $persistedSubject?->id
                    ?? self::nullableInt(data_get($taxonomyInference, 'best_match.subject_id')),
                'name' => $subSubjectName,
                'slug' => self::firstNonEmptyString([
                    $persistedSubSubject?->slug,
                    data_get($taxonomyInference, 'best_match.sub_subject_slug'),
                    data_get($generation->interpretation_payload, 'sub_subject_context.sub_subject_slug'),
                ]),
            ] : null,
            'grade_context' => [
                'jenjang' => self::firstNonEmptyString([
                    data_get($taxonomyInference, 'best_match.jenjang'),
                    data_get($taxonomyInference, 'prompt_context.jenjang'),
                ]),
                'kelas' => self::nullableScalarString(
                    data_get($taxonomyInference, 'best_match.kelas', data_get($taxonomyInference, 'prompt_context.kelas'))
                ),
                'semester' => self::nullableScalarString(
                    data_get($taxonomyInference, 'best_match.semester', data_get($taxonomyInference, 'prompt_context.semester'))
                ),
                'bab' => self::nullableScalarString(
                    data_get($taxonomyInference, 'best_match.bab', data_get($taxonomyInference, 'prompt_context.bab'))
                ),
            ],
            'content_guidance' => [
                'description' => self::firstNonEmptyString([
                    data_get($taxonomyInference, 'best_match.description'),
                ]),
                'structure' => self::firstNonEmptyString([
                    data_get($taxonomyInference, 'best_match.content_structure'),
                ]),
                'structure_items' => self::normalizeStringList(data_get($taxonomyInference, 'best_match.structure_items')),
            ],
            'matched_signals' => self::normalizeStringList(data_get($taxonomyInference, 'best_match.matched_signals')),
        ]);
    }

    /**
     * @param  array<string, mixed>|null  $payload
     * @return array<string, mixed>|null
     */
    public static function validate(?array $payload): ?array
    {
        if ($payload === null) {
            return null;
        }

        self::assertAllowedKeys(
            $payload,
            ['schema_version', 'source', 'confidence', 'subject', 'sub_subject', 'grade_context', 'content_guidance', 'matched_signals'],
            'payload'
        );

        if (isset($payload['confidence']) && is_array($payload['confidence'])) {
            self::assertAllowedKeys($payload['confidence'], ['score', 'label'], 'confidence');
        }

        if (isset($payload['subject']) && is_array($payload['subject'])) {
            self::assertAllowedKeys($payload['subject'], ['id', 'name', 'slug'], 'subject');
        }

        if (isset($payload['sub_subject']) && is_array($payload['sub_subject'])) {
            self::assertAllowedKeys($payload['sub_subject'], ['id', 'subject_id', 'name', 'slug'], 'sub_subject');
        }

        if (isset($payload['grade_context']) && is_array($payload['grade_context'])) {
            self::assertAllowedKeys($payload['grade_context'], ['jenjang', 'kelas', 'semester', 'bab'], 'grade_context');
        }

        if (isset($payload['content_guidance']) && is_array($payload['content_guidance'])) {
            self::assertAllowedKeys($payload['content_guidance'], ['description', 'structure', 'structure_items'], 'content_guidance');
        }

        $validator = Validator::make($payload, [
            'schema_version' => ['required', 'string', Rule::in([self::VERSION])],
            'source' => ['required', 'string', Rule::in(['submission_context', 'prompt_inference', 'interpretation_context'])],
            'confidence' => ['required', 'array'],
            'confidence.score' => ['nullable', 'numeric', 'between:0,1'],
            'confidence.label' => ['nullable', 'string', 'max:20'],
            'subject' => ['required', 'array'],
            'subject.id' => ['nullable', 'integer'],
            'subject.name' => ['required', 'string', 'max:100'],
            'subject.slug' => ['nullable', 'string', 'max:100'],
            'sub_subject' => ['nullable', 'array'],
            'sub_subject.id' => ['nullable', 'integer'],
            'sub_subject.subject_id' => ['nullable', 'integer'],
            'sub_subject.name' => ['required_with:sub_subject', 'string', 'max:100'],
            'sub_subject.slug' => ['nullable', 'string', 'max:100'],
            'grade_context' => ['required', 'array'],
            'grade_context.jenjang' => ['nullable', 'string', 'max:100'],
            'grade_context.kelas' => ['nullable', 'string', 'max:50'],
            'grade_context.semester' => ['nullable', 'string', 'max:50'],
            'grade_context.bab' => ['nullable', 'string', 'max:50'],
            'content_guidance' => ['required', 'array'],
            'content_guidance.description' => ['nullable', 'string', 'max:500'],
            'content_guidance.structure' => ['nullable', 'string', 'max:1000'],
            'content_guidance.structure_items' => ['present', 'array'],
            'content_guidance.structure_items.*' => ['string', 'max:200'],
            'matched_signals' => ['present', 'array'],
            'matched_signals.*' => ['string', 'max:100'],
        ]);

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Draft taxonomy hint payload failed validation.',
                MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
                ['errors' => $validator->errors()->toArray()]
            );
        }

        return [
            'schema_version' => self::VERSION,
            'source' => trim((string) $payload['source']),
            'confidence' => [
                'score' => isset($payload['confidence']['score']) && is_numeric($payload['confidence']['score'])
                    ? round((float) $payload['confidence']['score'], 4)
                    : null,
                'label' => self::firstNonEmptyString([$payload['confidence']['label'] ?? null]),
            ],
            'subject' => [
                'id' => self::nullableInt($payload['subject']['id'] ?? null),
                'name' => trim((string) $payload['subject']['name']),
                'slug' => self::firstNonEmptyString([$payload['subject']['slug'] ?? null]),
            ],
            'sub_subject' => is_array($payload['sub_subject'] ?? null) ? [
                'id' => self::nullableInt($payload['sub_subject']['id'] ?? null),
                'subject_id' => self::nullableInt($payload['sub_subject']['subject_id'] ?? null),
                'name' => trim((string) $payload['sub_subject']['name']),
                'slug' => self::firstNonEmptyString([$payload['sub_subject']['slug'] ?? null]),
            ] : null,
            'grade_context' => [
                'jenjang' => self::firstNonEmptyString([$payload['grade_context']['jenjang'] ?? null]),
                'kelas' => self::firstNonEmptyString([$payload['grade_context']['kelas'] ?? null]),
                'semester' => self::firstNonEmptyString([$payload['grade_context']['semester'] ?? null]),
                'bab' => self::firstNonEmptyString([$payload['grade_context']['bab'] ?? null]),
            ],
            'content_guidance' => [
                'description' => self::firstNonEmptyString([$payload['content_guidance']['description'] ?? null]),
                'structure' => self::firstNonEmptyString([$payload['content_guidance']['structure'] ?? null]),
                'structure_items' => self::normalizeStringList($payload['content_guidance']['structure_items'] ?? []),
            ],
            'matched_signals' => self::normalizeStringList($payload['matched_signals'] ?? []),
        ];
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
            'Draft taxonomy hint payload contains unsupported fields.',
            MediaGenerationErrorCode::LLM_CONTRACT_FAILED,
            [
                'path' => $path,
                'unknown_fields' => array_values($unknownKeys),
            ]
        );
    }

    /**
     * @param  array<int, mixed>  $values
     */
    private static function firstNonEmptyString(array $values): ?string
    {
        foreach ($values as $value) {
            if (! is_scalar($value)) {
                continue;
            }

            $normalized = trim((string) $value);

            if ($normalized !== '') {
                return $normalized;
            }
        }

        return null;
    }

    private static function nullableInt(mixed $value): ?int
    {
        return is_numeric($value) ? (int) $value : null;
    }

    private static function nullableScalarString(mixed $value): ?string
    {
        if (! is_scalar($value)) {
            return null;
        }

        $normalized = trim((string) $value);

        return $normalized !== '' ? $normalized : null;
    }

    /**
     * @return string[]
     */
    private static function normalizeStringList(mixed $values): array
    {
        if (! is_array($values)) {
            return [];
        }

        $normalized = [];

        foreach ($values as $value) {
            if (! is_scalar($value)) {
                continue;
            }

            $stringValue = trim((string) $value);

            if ($stringValue !== '') {
                $normalized[] = $stringValue;
            }
        }

        return array_values(array_unique($normalized));
    }
}