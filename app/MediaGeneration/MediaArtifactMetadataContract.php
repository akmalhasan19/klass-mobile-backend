<?php

namespace App\MediaGeneration;

use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;

final class MediaArtifactMetadataContract
{
    public const VERSION = 'media_generator_output_metadata.v1';

    private const CANONICAL_MIME_TYPES = [
        'pdf' => 'application/pdf',
        'docx' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
        'pptx' => 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    ];

    public static function validate(array $payload): array
    {
        self::assertAllowedKeys($payload, self::topLevelKeys(), 'payload');

        if (isset($payload['artifact_locator']) && is_array($payload['artifact_locator'])) {
            self::assertAllowedKeys($payload['artifact_locator'], ['kind', 'value'], 'artifact_locator');
        }

        if (isset($payload['generator']) && is_array($payload['generator'])) {
            self::assertAllowedKeys($payload['generator'], ['name', 'version'], 'generator');
        }

        $payload = self::applyDefaults($payload);

        $validator = Validator::make($payload, [
            'schema_version' => ['required', 'string', Rule::in([self::VERSION])],
            'export_format' => ['required', 'string', Rule::in(MediaPromptInterpretationSchema::allowedOutputFormats())],
            'title' => ['required', 'string', 'max:200'],
            'filename' => ['required', 'string', 'max:255'],
            'extension' => ['required', 'string', Rule::in(MediaPromptInterpretationSchema::allowedOutputFormats())],
            'mime_type' => ['required', 'string', 'max:255'],
            'size_bytes' => ['required', 'integer', 'min:1'],
            'checksum_sha256' => ['required', 'string', 'regex:/^[A-Fa-f0-9]{64}$/'],
            'page_count' => ['nullable', 'integer', 'min:1'],
            'slide_count' => ['nullable', 'integer', 'min:1'],
            'artifact_locator' => ['required', 'array'],
            'artifact_locator.kind' => ['required', 'string', Rule::in(['temporary_path', 'signed_url', 'storage_object'])],
            'artifact_locator.value' => ['required', 'string', 'max:2048'],
            'generator' => ['required', 'array'],
            'generator.name' => ['required', 'string', 'max:100'],
            'generator.version' => ['required', 'string', 'max:50'],
            'warnings' => ['present', 'array'],
            'warnings.*' => ['string', 'max:500'],
        ]);

        if ($validator->fails()) {
            throw new MediaGenerationContractException(
                'Python generator metadata failed validation.',
                'artifact_invalid',
                ['errors' => $validator->errors()->toArray()]
            );
        }

        $normalized = [
            'schema_version' => self::VERSION,
            'export_format' => trim($payload['export_format']),
            'title' => trim($payload['title']),
            'filename' => trim($payload['filename']),
            'extension' => trim($payload['extension']),
            'mime_type' => trim($payload['mime_type']),
            'size_bytes' => (int) $payload['size_bytes'],
            'checksum_sha256' => strtolower(trim($payload['checksum_sha256'])),
            'page_count' => $payload['page_count'] !== null ? (int) $payload['page_count'] : null,
            'slide_count' => $payload['slide_count'] !== null ? (int) $payload['slide_count'] : null,
            'artifact_locator' => [
                'kind' => trim($payload['artifact_locator']['kind']),
                'value' => trim($payload['artifact_locator']['value']),
            ],
            'generator' => [
                'name' => trim($payload['generator']['name']),
                'version' => trim($payload['generator']['version']),
            ],
            'warnings' => array_values($payload['warnings']),
        ];

        self::assertCrossFieldConsistency($normalized);

        return $normalized;
    }

    private static function applyDefaults(array $payload): array
    {
        if (! array_key_exists('warnings', $payload)) {
            $payload['warnings'] = [];
        }

        if (! array_key_exists('page_count', $payload)) {
            $payload['page_count'] = null;
        }

        if (! array_key_exists('slide_count', $payload)) {
            $payload['slide_count'] = null;
        }

        return $payload;
    }

    private static function assertCrossFieldConsistency(array $payload): void
    {
        if ($payload['extension'] !== $payload['export_format']) {
            throw new MediaGenerationContractException(
                'Artifact extension must match export format.',
                'artifact_invalid',
                ['extension' => $payload['extension'], 'export_format' => $payload['export_format']]
            );
        }

        $filenameExtension = strtolower(pathinfo($payload['filename'], PATHINFO_EXTENSION));

        if ($filenameExtension !== $payload['extension']) {
            throw new MediaGenerationContractException(
                'Artifact filename must match the declared extension.',
                'artifact_invalid',
                ['filename' => $payload['filename'], 'extension' => $payload['extension']]
            );
        }

        $expectedMimeType = self::CANONICAL_MIME_TYPES[$payload['export_format']] ?? null;

        if ($expectedMimeType === null || $payload['mime_type'] !== $expectedMimeType) {
            throw new MediaGenerationContractException(
                'Artifact mime type must match the declared export format.',
                'artifact_invalid',
                ['mime_type' => $payload['mime_type'], 'export_format' => $payload['export_format']]
            );
        }

        if ($payload['export_format'] === 'pptx' && $payload['slide_count'] === null) {
            throw new MediaGenerationContractException(
                'PPTX metadata must include slide_count.',
                'artifact_invalid'
            );
        }

        if ($payload['export_format'] !== 'pptx' && $payload['slide_count'] !== null) {
            throw new MediaGenerationContractException(
                'Non-PPTX metadata must not include slide_count.',
                'artifact_invalid'
            );
        }
    }

    private static function assertAllowedKeys(array $payload, array $allowedKeys, string $path): void
    {
        $unknownKeys = array_diff(array_keys($payload), $allowedKeys);

        if ($unknownKeys === []) {
            return;
        }

        throw new MediaGenerationContractException(
            'Python generator metadata contains unsupported fields.',
            'artifact_invalid',
            [
                'path' => $path,
                'unknown_fields' => array_values($unknownKeys),
            ]
        );
    }

    private static function topLevelKeys(): array
    {
        return [
            'schema_version',
            'export_format',
            'title',
            'filename',
            'extension',
            'mime_type',
            'size_bytes',
            'checksum_sha256',
            'page_count',
            'slide_count',
            'artifact_locator',
            'generator',
            'warnings',
        ];
    }
}