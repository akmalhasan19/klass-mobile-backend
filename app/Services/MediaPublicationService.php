<?php

namespace App\Services;

use App\MediaGeneration\MediaArtifactMetadataContract;
use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationServiceException;
use App\Models\Content;
use App\Models\MediaGeneration;
use App\Models\RecommendedProject;
use App\Models\Subject;
use App\Models\SubSubject;
use App\Models\Topic;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Throwable;

class MediaPublicationService
{
    public function __construct(
        protected ?FileUploadService $fileUploadService = null,
        protected ?ThumbnailGeneratorService $thumbnailGeneratorService = null,
    ) {
    }

    public function publish(MediaGeneration $generation, ?callable $afterArtifactPrepared = null): MediaGeneration
    {
        $preparedArtifact = $this->prepareArtifactForPublication($generation);

        try {
            if ($afterArtifactPrepared !== null) {
                $callbackResult = $afterArtifactPrepared($generation->fresh() ?? $generation, $preparedArtifact);

                if ($callbackResult instanceof MediaGeneration) {
                    $generation = $callbackResult;
                }
            }

            return DB::transaction(function () use ($generation, $preparedArtifact): MediaGeneration {
                /** @var MediaGeneration $lockedGeneration */
                $lockedGeneration = MediaGeneration::query()
                    ->with(['teacher', 'subject', 'subSubject.subject'])
                    ->lockForUpdate()
                    ->findOrFail($generation->getKey());

                $lockedGeneration->forceFill([
                    'storage_path' => $preparedArtifact['storage_path'] ?? $lockedGeneration->storage_path,
                    'file_url' => $preparedArtifact['file_url'] ?? $lockedGeneration->file_url,
                    'thumbnail_url' => $preparedArtifact['thumbnail_url'] ?? $lockedGeneration->thumbnail_url,
                    'mime_type' => $preparedArtifact['mime_type'] ?? $lockedGeneration->mime_type,
                    'error_code' => null,
                    'error_message' => null,
                ])->save();

                $existingProject = $this->resolveExistingRecommendedProject($lockedGeneration);
                $existingTopic = $this->resolveExistingTopic($lockedGeneration, $existingProject);
                $existingContent = $this->resolveExistingContent($lockedGeneration, $existingProject, $existingTopic);

                $topic = $existingTopic ?? $this->createTopic($lockedGeneration);
                $content = $existingContent ?? $this->createContent($lockedGeneration, $topic);
                $project = $existingProject ?? $this->createRecommendedProject($lockedGeneration, $topic, $content);

                $deliveryPayload = $this->buildDeliveryPayload($lockedGeneration, $topic, $content, $project);

                $lockedGeneration->forceFill([
                    'topic_id' => $topic->id,
                    'content_id' => $content->id,
                    'recommended_project_id' => $project->id,
                    'delivery_payload' => $deliveryPayload,
                ])->save();

                return $lockedGeneration->fresh(['topic', 'content', 'recommendedProject', 'subject', 'subSubject.subject']);
            });
        } catch (Throwable $throwable) {
            $this->compensateUploadedFiles($preparedArtifact['uploaded_paths']);

            if ($throwable instanceof MediaGenerationServiceException) {
                throw $throwable;
            }

            throw MediaGenerationServiceException::publicationFailed(
                'Publishing generated media failed.',
                ['exception' => $throwable->getMessage()]
            );
        } finally {
            $this->cleanupTempFiles($preparedArtifact['cleanup_paths']);
        }
    }

    /**
     * @return array{storage_path: string|null, file_url: string|null, thumbnail_url: string|null, mime_type: string|null, uploaded_paths: string[], cleanup_paths: string[]}
     */
    protected function prepareArtifactForPublication(MediaGeneration $generation): array
    {
        $uploadedPaths = [];
        $cleanupPaths = [];

        try {
            $artifactMetadata = $this->resolveArtifactMetadata($generation);
            $storagePath = $generation->storage_path;
            $fileUrl = $generation->file_url;
            $thumbnailUrl = $generation->thumbnail_url;
            $mimeType = $generation->mime_type ?: data_get($artifactMetadata, 'mime_type');
            $artifactSource = [
                'local_path' => null,
                'cleanup_paths' => [],
            ];

            if (! $this->hasStoredArtifact($generation)) {
                $artifactSource = $this->resolveArtifactSource($generation, $artifactMetadata);
                $cleanupPaths = array_merge($cleanupPaths, $artifactSource['cleanup_paths']);
                $this->validateArtifactForPublication($artifactMetadata, $artifactSource);

                $upload = $this->uploadArtifact($artifactMetadata, $artifactSource);
                $uploadedPaths[] = $upload['path'];
                $storagePath = $upload['path'];
                $fileUrl = $upload['url'];
            }

            if ($thumbnailUrl === null || trim((string) $thumbnailUrl) === '') {
                $thumbnailLocalPath = $this->generateThumbnail(
                    localArtifactPath: $artifactSource['local_path'],
                    fileUrl: $fileUrl,
                    outputType: (string) ($generation->resolved_output_type ?: data_get($artifactMetadata, 'extension', 'file')),
                    title: (string) (data_get($artifactMetadata, 'title') ?: $this->resolvePublicationTitle($generation)),
                );

                if ($thumbnailLocalPath !== null) {
                    $cleanupPaths[] = $thumbnailLocalPath;
                    $thumbnailUpload = $this->fileUploadService()->uploadFromPath(
                        $thumbnailLocalPath,
                        'generated_thumbnail_' . pathinfo((string) ($storagePath ?? basename($thumbnailLocalPath)), PATHINFO_FILENAME) . '.' . pathinfo($thumbnailLocalPath, PATHINFO_EXTENSION),
                        'gallery',
                    );
                    $uploadedPaths[] = $thumbnailUpload['path'];
                    $thumbnailUrl = $thumbnailUpload['url'];
                }
            }

            return [
                'storage_path' => $storagePath,
                'file_url' => $fileUrl,
                'thumbnail_url' => $thumbnailUrl,
                'mime_type' => $mimeType,
                'uploaded_paths' => $uploadedPaths,
                'cleanup_paths' => $cleanupPaths,
            ];
        } catch (Throwable $throwable) {
            $this->compensateUploadedFiles($uploadedPaths);
            $this->cleanupTempFiles($cleanupPaths);

            if ($throwable instanceof MediaGenerationServiceException) {
                throw $throwable;
            }

            if ($throwable instanceof MediaGenerationContractException) {
                throw MediaGenerationServiceException::artifactInvalid(
                    'Generated artifact payload is invalid for publication.',
                    ['exception' => $throwable->getMessage(), 'context' => $throwable->context()]
                );
            }

            throw MediaGenerationServiceException::uploadFailed(
                'Preparing generated artifact for publication failed.',
                ['exception' => $throwable->getMessage()]
            );
        }
    }

    protected function hasStoredArtifact(MediaGeneration $generation): bool
    {
        return is_string($generation->storage_path)
            && trim($generation->storage_path) !== ''
            && is_string($generation->file_url)
            && trim($generation->file_url) !== '';
    }

    /**
     * @return array<string, mixed>|null
     */
    protected function resolveArtifactMetadata(MediaGeneration $generation): ?array
    {
        $payload = data_get($generation->generator_service_response, 'response.artifact_metadata')
            ?? data_get($generation->generator_service_response, 'artifact_metadata');

        if (! is_array($payload)) {
            return null;
        }

        return MediaArtifactMetadataContract::validate($payload);
    }

    /**
     * @param  array<string, mixed>|null  $artifactMetadata
     * @return array{local_path: string|null, cleanup_paths: string[]}
     */
    protected function resolveArtifactSource(MediaGeneration $generation, ?array $artifactMetadata): array
    {
        if (! is_array($artifactMetadata)) {
            return [
                'local_path' => null,
                'cleanup_paths' => [],
            ];
        }

        $kind = data_get($artifactMetadata, 'artifact_locator.kind');
        $value = data_get($artifactMetadata, 'artifact_locator.value');

        if (! is_string($kind) || ! is_string($value) || trim($value) === '') {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact locator is missing from generator metadata.'
            );
        }

        return match ($kind) {
            'temporary_path' => $this->artifactSourceFromTemporaryPath($value),
            'signed_url' => $this->artifactSourceFromSignedUrl($value, (string) data_get($artifactMetadata, 'filename', 'generated-artifact')),
            'storage_object' => [
                'local_path' => null,
                'cleanup_paths' => [],
            ],
            default => throw MediaGenerationServiceException::artifactInvalid(
                'Unsupported artifact locator kind.',
                ['kind' => $kind]
            ),
        };
    }

    /**
     * @return array{local_path: string, cleanup_paths: string[]}
     */
    protected function artifactSourceFromTemporaryPath(string $path): array
    {
        if (! is_file($path)) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Temporary artifact path does not exist.',
                ['path' => $path]
            );
        }

        return [
            'local_path' => $path,
            'cleanup_paths' => [],
        ];
    }

    /**
     * @return array{local_path: string, cleanup_paths: string[]}
     */
    protected function artifactSourceFromSignedUrl(string $url, string $filename): array
    {
        $extension = strtolower(pathinfo($filename, PATHINFO_EXTENSION) ?: pathinfo(parse_url($url, PHP_URL_PATH) ?: '', PATHINFO_EXTENSION) ?: 'bin');
        $tempPath = sys_get_temp_dir() . '/' . 'generated_artifact_' . Str::random(12) . '.' . $extension;
        $contents = @file_get_contents($url);

        if ($contents === false) {
            throw MediaGenerationServiceException::uploadFailed(
                'Could not download signed artifact URL for publication.',
                ['url' => $url]
            );
        }

        file_put_contents($tempPath, $contents);

        return [
            'local_path' => $tempPath,
            'cleanup_paths' => [$tempPath],
        ];
    }

    /**
     * @param  array<string, mixed>|null  $artifactMetadata
     * @param  array{local_path: string|null, cleanup_paths: string[]}  $artifactSource
     * @return array{path: string, url: string}
     */
    protected function uploadArtifact(?array $artifactMetadata, array $artifactSource): array
    {
        if (is_array($artifactMetadata)
            && data_get($artifactMetadata, 'artifact_locator.kind') === 'storage_object'
            && is_string(data_get($artifactMetadata, 'artifact_locator.value'))
            && trim((string) data_get($artifactMetadata, 'artifact_locator.value')) !== '') {
            $path = trim((string) data_get($artifactMetadata, 'artifact_locator.value'));

            return [
                'path' => $path,
                'url' => $this->fileUploadService()->generatePublicUrl($path),
            ];
        }

        $localPath = $artifactSource['local_path'];

        if (! is_string($localPath) || trim($localPath) === '' || ! is_file($localPath)) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact file is not available for upload.'
            );
        }

        $originalName = is_array($artifactMetadata)
            ? (string) data_get($artifactMetadata, 'filename', basename($localPath))
            : basename($localPath);

        return $this->fileUploadService()->uploadFromPath($localPath, $originalName, 'materials');
    }

    protected function validateArtifactForPublication(?array $artifactMetadata, array $artifactSource): void
    {
        if (! is_array($artifactMetadata)) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact metadata is required before publication.'
            );
        }

        $locatorKind = (string) data_get($artifactMetadata, 'artifact_locator.kind', '');

        if ($locatorKind === 'storage_object') {
            $storagePath = trim((string) data_get($artifactMetadata, 'artifact_locator.value', ''));

            if ($storagePath === '' || ! $this->fileUploadService()->exists($storagePath)) {
                throw MediaGenerationServiceException::artifactInvalid(
                    'Stored artifact reference does not exist.',
                    ['storage_path' => $storagePath]
                );
            }

            return;
        }

        $localPath = $artifactSource['local_path'] ?? null;

        if (! is_string($localPath) || trim($localPath) === '' || ! is_file($localPath)) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact file is not available for validation.'
            );
        }

        $expectedExtension = strtolower((string) data_get($artifactMetadata, 'extension', ''));
        $pathExtension = strtolower(pathinfo($localPath, PATHINFO_EXTENSION));

        if ($expectedExtension === '' || $pathExtension !== $expectedExtension) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact file extension does not match the expected format.',
                [
                    'expected_extension' => $expectedExtension,
                    'actual_extension' => $pathExtension,
                    'path' => $localPath,
                ]
            );
        }

        $actualSize = filesize($localPath);
        $expectedSize = (int) data_get($artifactMetadata, 'size_bytes', 0);

        if (! is_int($actualSize) || $actualSize < 1) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact file is empty.',
                ['path' => $localPath]
            );
        }

        if ($actualSize !== $expectedSize) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact file size does not match generator metadata.',
                [
                    'expected_size_bytes' => $expectedSize,
                    'actual_size_bytes' => $actualSize,
                    'path' => $localPath,
                ]
            );
        }

        $actualChecksum = hash_file('sha256', $localPath);
        $expectedChecksum = strtolower((string) data_get($artifactMetadata, 'checksum_sha256', ''));

        if (! is_string($actualChecksum) || strtolower($actualChecksum) !== $expectedChecksum) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact checksum does not match generator metadata.',
                ['path' => $localPath]
            );
        }

        $this->validateArtifactMimeType($artifactMetadata, $localPath);
        $this->validateArtifactStructure($expectedExtension, $localPath);
    }

    protected function validateArtifactMimeType(array $artifactMetadata, string $localPath): void
    {
        $expectedExtension = strtolower((string) data_get($artifactMetadata, 'extension', ''));
        $expectedMimeType = $this->canonicalMimeTypeForExtension($expectedExtension);
        $declaredMimeType = strtolower((string) data_get($artifactMetadata, 'mime_type', ''));

        if ($expectedMimeType === null || $declaredMimeType !== $expectedMimeType) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact mime type does not match the expected format.',
                [
                    'expected_mime_type' => $expectedMimeType,
                    'declared_mime_type' => $declaredMimeType,
                ]
            );
        }

        $detectedMimeType = mime_content_type($localPath);

        if (is_string($detectedMimeType)
            && trim($detectedMimeType) !== ''
            && ! in_array(strtolower($detectedMimeType), $this->allowedDetectedMimeTypes($expectedExtension), true)) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Artifact mime type detection failed for the expected format.',
                [
                    'expected_extension' => $expectedExtension,
                    'detected_mime_type' => $detectedMimeType,
                    'path' => $localPath,
                ]
            );
        }
    }

    protected function validateArtifactStructure(string $extension, string $localPath): void
    {
        match ($extension) {
            'pdf' => $this->assertPdfArtifactIsReadable($localPath),
            'docx' => $this->assertOfficeArtifactEntries($localPath, ['[Content_Types].xml', 'word/document.xml']),
            'pptx' => $this->assertOfficeArtifactEntries($localPath, ['[Content_Types].xml', 'ppt/presentation.xml']),
            default => throw MediaGenerationServiceException::artifactInvalid(
                'Artifact format is not supported for publication validation.',
                ['extension' => $extension]
            ),
        };
    }

    /**
     * @param  string[]  $requiredEntries
     */
    protected function assertOfficeArtifactEntries(string $localPath, array $requiredEntries): void
    {
        if (! class_exists(\ZipArchive::class)) {
            $signature = file_get_contents($localPath, false, null, 0, 2);

            if ($signature !== 'PK') {
                throw MediaGenerationServiceException::artifactInvalid(
                    'Office artifact is corrupt and cannot be opened as an archive.',
                    ['path' => $localPath]
                );
            }

            return;
        }

        $zip = new \ZipArchive();
        $result = $zip->open($localPath);

        if ($result !== true) {
            throw MediaGenerationServiceException::artifactInvalid(
                'Office artifact is corrupt and cannot be opened as an archive.',
                ['path' => $localPath, 'zip_result' => $result]
            );
        }

        try {
            foreach ($requiredEntries as $requiredEntry) {
                if ($zip->locateName($requiredEntry) === false) {
                    throw MediaGenerationServiceException::artifactInvalid(
                        'Office artifact is missing required package entries.',
                        ['path' => $localPath, 'missing_entry' => $requiredEntry]
                    );
                }
            }
        } finally {
            $zip->close();
        }
    }

    protected function assertPdfArtifactIsReadable(string $localPath): void
    {
        $header = file_get_contents($localPath, false, null, 0, 5);
        $tail = $this->tailContents($localPath, 1024);

        if (! is_string($header) || ! str_starts_with($header, '%PDF-')) {
            throw MediaGenerationServiceException::artifactInvalid(
                'PDF artifact does not contain a valid header.',
                ['path' => $localPath]
            );
        }

        if ($tail === null || ! str_contains($tail, '%%EOF')) {
            throw MediaGenerationServiceException::artifactInvalid(
                'PDF artifact appears truncated or corrupt.',
                ['path' => $localPath]
            );
        }
    }

    protected function tailContents(string $localPath, int $length): ?string
    {
        $handle = @fopen($localPath, 'rb');

        if ($handle === false) {
            return null;
        }

        try {
            $size = filesize($localPath);

            if (! is_int($size) || $size < 1) {
                return null;
            }

            $offset = max(0, $size - $length);
            fseek($handle, $offset);

            $contents = stream_get_contents($handle);

            return is_string($contents) ? $contents : null;
        } finally {
            fclose($handle);
        }
    }

    protected function canonicalMimeTypeForExtension(string $extension): ?string
    {
        return match ($extension) {
            'pdf' => 'application/pdf',
            'docx' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
            'pptx' => 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
            default => null,
        };
    }

    /**
     * @return string[]
     */
    protected function allowedDetectedMimeTypes(string $extension): array
    {
        return match ($extension) {
            'pdf' => ['application/pdf', 'application/x-pdf'],
            'docx', 'pptx' => [
                'application/octet-stream',
                'application/zip',
                'application/x-zip',
                'application/x-zip-compressed',
                $this->canonicalMimeTypeForExtension($extension),
            ],
            default => [],
        };
    }

    protected function generateThumbnail(?string $localArtifactPath, ?string $fileUrl, string $outputType, string $title): ?string
    {
        try {
            if (is_string($localArtifactPath) && trim($localArtifactPath) !== '' && is_file($localArtifactPath)) {
                $thumbnailPath = $this->thumbnailGeneratorService()->generateFromFile($localArtifactPath);

                if ($thumbnailPath !== null) {
                    return $thumbnailPath;
                }
            }

            if (is_string($fileUrl) && trim($fileUrl) !== '') {
                $thumbnailPath = $this->thumbnailGeneratorService()->generateFromUrl($fileUrl);

                if ($thumbnailPath !== null) {
                    return $thumbnailPath;
                }
            }
        } catch (Throwable $throwable) {
            report($throwable);
        }

        try {
            return $this->thumbnailGeneratorService()->generateFallbackVisual($outputType, $title);
        } catch (Throwable $throwable) {
            report($throwable);

            return null;
        }
    }

    /**
     * @param  string[]  $uploadedPaths
     */
    protected function compensateUploadedFiles(array $uploadedPaths): void
    {
        foreach (array_reverse($uploadedPaths) as $path) {
            if (! is_string($path) || trim($path) === '') {
                continue;
            }

            try {
                $this->fileUploadService()->delete($path);
            } catch (Throwable $throwable) {
                report($throwable);
            }
        }
    }

    /**
     * @param  string[]  $cleanupPaths
     */
    protected function cleanupTempFiles(array $cleanupPaths): void
    {
        foreach ($cleanupPaths as $path) {
            if (is_string($path) && trim($path) !== '' && is_file($path)) {
                @unlink($path);
            }
        }
    }

    protected function fileUploadService(): FileUploadService
    {
        return $this->fileUploadService ??= app(FileUploadService::class);
    }

    protected function thumbnailGeneratorService(): ThumbnailGeneratorService
    {
        return $this->thumbnailGeneratorService ??= app(ThumbnailGeneratorService::class);
    }

    protected function resolveExistingTopic(MediaGeneration $generation, ?RecommendedProject $project): ?Topic
    {
        if ($generation->topic_id) {
            $topic = Topic::query()->find($generation->topic_id);

            if ($topic) {
                return $topic;
            }
        }

        $topicId = data_get($project?->source_payload, 'topic_id')
            ?? data_get($generation->delivery_payload, 'publication.topic.id');

        return $topicId ? Topic::query()->find($topicId) : null;
    }

    protected function resolveExistingContent(
        MediaGeneration $generation,
        ?RecommendedProject $project,
        ?Topic $topic,
    ): ?Content {
        if ($generation->content_id) {
            $content = Content::query()->find($generation->content_id);

            if ($content) {
                return $content;
            }
        }

        $contentId = data_get($project?->source_payload, 'content_id')
            ?? data_get($generation->delivery_payload, 'publication.content.id');

        if ($contentId) {
            $content = Content::query()->find($contentId);

            if ($content) {
                return $content;
            }
        }

        if (! $topic) {
            return null;
        }

        return Content::query()
            ->where('topic_id', $topic->id)
            ->get()
            ->first(fn (Content $content): bool => data_get($content->data, 'media_generation_id') === $generation->id);
    }

    protected function resolveExistingRecommendedProject(MediaGeneration $generation): ?RecommendedProject
    {
        if ($generation->recommended_project_id) {
            $project = RecommendedProject::query()->find($generation->recommended_project_id);

            if ($project) {
                return $project;
            }
        }

        return RecommendedProject::query()
            ->where('source_type', RecommendedProject::SOURCE_AI_GENERATED)
            ->get()
            ->first(fn (RecommendedProject $project): bool => data_get($project->source_payload, 'media_generation_id') === $generation->id);
    }

    protected function createTopic(MediaGeneration $generation): Topic
    {
        return Topic::create([
            'title' => $this->resolvePublicationTitle($generation),
            'teacher_id' => (string) $generation->teacher_id,
            'sub_subject_id' => $this->resolveSubSubjectId($generation),
            'thumbnail_url' => $generation->thumbnail_url,
            'is_published' => true,
            'order' => 0,
        ]);
    }

    protected function createContent(MediaGeneration $generation, Topic $topic): Content
    {
        return Content::create([
            'topic_id' => $topic->id,
            'type' => 'brief',
            'title' => $this->resolvePublicationTitle($generation),
            'data' => $this->buildContentData($generation, $topic),
            'media_url' => $generation->file_url,
            'is_published' => true,
            'order' => 0,
        ]);
    }

    protected function createRecommendedProject(MediaGeneration $generation, Topic $topic, Content $content): RecommendedProject
    {
        $project = RecommendedProject::create([
            'title' => $this->resolvePublicationTitle($generation),
            'description' => $this->resolvePublicationDescription($generation),
            'thumbnail_url' => $generation->thumbnail_url,
            'project_file_url' => $generation->file_url,
            'ratio' => '16:9',
            'project_type' => 'learning_material',
            'tags' => $this->buildProjectTags($generation),
            'modules' => $this->buildProjectModules($generation),
            'source_type' => RecommendedProject::SOURCE_AI_GENERATED,
            'source_reference' => null,
            'source_payload' => [],
            'display_priority' => 0,
            'is_active' => true,
            'starts_at' => null,
            'ends_at' => null,
            'created_by' => $generation->teacher_id,
            'updated_by' => $generation->teacher_id,
        ]);

        $project->forceFill([
            'source_reference' => (string) $project->id,
            'source_payload' => $this->buildProjectSourcePayload($generation, $topic, $content, $project),
        ])->save();

        return $project->fresh();
    }

    protected function buildContentData(MediaGeneration $generation, Topic $topic): array
    {
        return [
            'media_generation_id' => $generation->id,
            'teacher_id' => (string) $generation->teacher_id,
            'topic_id' => $topic->id,
            'subject_id' => $this->resolveSubjectId($generation),
            'sub_subject_id' => $this->resolveSubSubjectId($generation),
            'output_type' => $this->resolveOutputType($generation),
            'mime_type' => $generation->mime_type,
            'storage_path' => $generation->storage_path,
            'file_url' => $generation->file_url,
            'thumbnail_url' => $generation->thumbnail_url,
            'summary' => $this->resolvePublicationDescription($generation),
            'teacher_delivery_summary' => data_get($generation->generation_spec_payload, 'teacher_delivery_summary')
                ?? data_get($generation->interpretation_payload, 'teacher_delivery_summary'),
            'section_titles' => $this->buildProjectModules($generation),
        ];
    }

    protected function buildProjectSourcePayload(
        MediaGeneration $generation,
        Topic $topic,
        Content $content,
        RecommendedProject $project,
    ): array {
        $taxonomy = $this->buildTaxonomy($generation);

        return [
            'media_generation_id' => $generation->id,
            'source_reference' => (string) $project->id,
            'topic_id' => $topic->id,
            'content_id' => $content->id,
            'teacher_id' => (string) $generation->teacher_id,
            'owner_user_id' => $topic->owner_user_id,
            'subject_id' => $this->resolveSubjectId($generation),
            'sub_subject_id' => $this->resolveSubSubjectId($generation),
            'taxonomy' => $taxonomy,
            'personalization' => $topic->resolvePersonalizationContext(),
            'output_type' => $this->resolveOutputType($generation),
            'mime_type' => $generation->mime_type,
            'file_url' => $generation->file_url,
            'thumbnail_url' => $generation->thumbnail_url,
            'score' => $this->resolveRecommendationScore($generation),
        ];
    }

    protected function buildDeliveryPayload(
        MediaGeneration $generation,
        Topic $topic,
        Content $content,
        RecommendedProject $project,
    ): array {
        return [
            'media_generation_id' => $generation->id,
            'artifact' => [
                'output_type' => $this->resolveOutputType($generation),
                'storage_path' => $generation->storage_path,
                'file_url' => $generation->file_url,
                'thumbnail_url' => $generation->thumbnail_url,
                'mime_type' => $generation->mime_type,
            ],
            'publication' => [
                'topic' => [
                    'id' => $topic->id,
                    'title' => $topic->title,
                ],
                'content' => [
                    'id' => $content->id,
                    'type' => $content->type,
                    'title' => $content->title,
                    'media_url' => $content->media_url,
                ],
                'recommended_project' => [
                    'id' => (string) $project->id,
                    'source_type' => $project->source_type,
                    'source_reference' => $project->source_reference,
                    'project_file_url' => $project->project_file_url,
                ],
            ],
            'summary' => [
                'title' => $this->resolvePublicationTitle($generation),
                'preview' => $this->resolvePublicationDescription($generation),
            ],
        ];
    }

    protected function buildProjectModules(MediaGeneration $generation): array
    {
        $sections = data_get($generation->generation_spec_payload, 'sections', []);

        if (is_array($sections) && $sections !== []) {
            $titles = collect($sections)
                ->map(fn (mixed $section): ?string => is_array($section) ? trim((string) data_get($section, 'title')) : null)
                ->filter()
                ->values()
                ->all();

            if ($titles !== []) {
                return $titles;
            }
        }

        $blueprintSections = data_get($generation->interpretation_payload, 'document_blueprint.sections', []);

        return collect(is_array($blueprintSections) ? $blueprintSections : [])
            ->map(fn (mixed $section): ?string => is_array($section) ? trim((string) data_get($section, 'title')) : null)
            ->filter()
            ->values()
            ->all();
    }

    protected function buildProjectTags(MediaGeneration $generation): array
    {
        $resolvedTaxonomy = $this->resolvePublicationTaxonomy($generation);
        $tags = [
            $resolvedTaxonomy['subject']?->name ?? data_get($generation->interpretation_payload, 'subject_context.subject_name'),
            $resolvedTaxonomy['sub_subject']?->name ?? data_get($generation->interpretation_payload, 'sub_subject_context.sub_subject_name'),
            strtoupper($this->resolveOutputType($generation)),
        ];

        return collect($tags)
            ->filter(fn (mixed $tag): bool => is_string($tag) && trim($tag) !== '')
            ->map(fn (string $tag): string => trim($tag))
            ->unique()
            ->values()
            ->all();
    }

    protected function buildTaxonomy(MediaGeneration $generation): ?array
    {
        $resolvedTaxonomy = $this->resolvePublicationTaxonomy($generation);
        $subject = $resolvedTaxonomy['subject'];
        $subSubject = $resolvedTaxonomy['sub_subject'];

        if (! $subSubject) {
            return null;
        }

        return [
            'subject' => $subject ? [
                'id' => $subject->id,
                'name' => $subject->name,
                'slug' => $subject->slug,
            ] : null,
            'sub_subject' => [
                'id' => $subSubject->id,
                'subject_id' => $subSubject->subject_id,
                'name' => $subSubject->name,
                'slug' => $subSubject->slug,
            ],
        ];
    }

    protected function resolveSubjectId(MediaGeneration $generation): ?int
    {
        $resolvedTaxonomy = $this->resolvePublicationTaxonomy($generation);

        return $resolvedTaxonomy['subject']?->id ?? $resolvedTaxonomy['sub_subject']?->subject_id;
    }

    protected function resolveSubSubjectId(MediaGeneration $generation): ?int
    {
        return $this->resolvePublicationTaxonomy($generation)['sub_subject']?->id;
    }

    /**
     * @return array{subject: Subject|null, sub_subject: SubSubject|null}
     */
    protected function resolvePublicationTaxonomy(MediaGeneration $generation): array
    {
        $subSubject = $generation->subSubject;
        $subject = $subSubject?->subject ?? $generation->subject;

        if ($subSubject !== null && $subject === null) {
            $subSubject->loadMissing('subject');
            $subject = $subSubject->subject;
        }

        if ($subject !== null || $subSubject !== null) {
            return [
                'subject' => $subject,
                'sub_subject' => $subSubject,
            ];
        }

        $inferredSubSubjectId = data_get($generation->interpretation_audit_payload, 'taxonomy_inference.best_match.sub_subject_id');
        $inferredSubjectId = data_get($generation->interpretation_audit_payload, 'taxonomy_inference.best_match.subject_id');

        if (is_numeric($inferredSubSubjectId)) {
            $subSubject = SubSubject::query()
                ->with('subject')
                ->find((int) $inferredSubSubjectId);

            if ($subSubject !== null) {
                $subject = $subSubject->subject;
                $generation->setRelation('subSubject', $subSubject);
                $generation->setRelation('subject', $subject);

                return [
                    'subject' => $subject,
                    'sub_subject' => $subSubject,
                ];
            }
        }

        if (is_numeric($inferredSubjectId)) {
            $subject = Subject::query()->find((int) $inferredSubjectId);

            if ($subject !== null) {
                $generation->setRelation('subject', $subject);
            }
        }

        return [
            'subject' => $subject,
            'sub_subject' => null,
        ];
    }

    protected function resolveOutputType(MediaGeneration $generation): string
    {
        if (is_string($generation->resolved_output_type) && trim($generation->resolved_output_type) !== '') {
            return trim($generation->resolved_output_type);
        }

        $exportFormat = data_get($generation->generation_spec_payload, 'export_format');

        if (is_string($exportFormat) && trim($exportFormat) !== '') {
            return trim($exportFormat);
        }

        return MediaGeneration::normalizePreferredOutputType($generation->preferred_output_type);
    }

    protected function resolvePublicationTitle(MediaGeneration $generation): string
    {
        $candidates = [
            data_get($generation->generation_spec_payload, 'title'),
            data_get($generation->interpretation_payload, 'document_blueprint.title'),
            data_get($generation->delivery_payload, 'summary.title'),
        ];

        foreach ($candidates as $candidate) {
            if (is_string($candidate) && trim($candidate) !== '') {
                return trim($candidate);
            }
        }

        return Str::limit(Str::of($generation->raw_prompt)->squish()->toString(), 120, '');
    }

    protected function resolvePublicationDescription(MediaGeneration $generation): ?string
    {
        $candidates = [
            data_get($generation->generation_spec_payload, 'teacher_delivery_summary'),
            data_get($generation->interpretation_payload, 'teacher_delivery_summary'),
            data_get($generation->generation_spec_payload, 'summary'),
        ];

        foreach ($candidates as $candidate) {
            if (is_string($candidate) && trim($candidate) !== '') {
                return trim($candidate);
            }
        }

        return null;
    }

    protected function resolveRecommendationScore(MediaGeneration $generation): float
    {
        $confidenceScore = data_get($generation->interpretation_payload, 'confidence.score');

        if (is_numeric($confidenceScore)) {
            return round((float) $confidenceScore, 4);
        }

        $candidateScore = data_get($generation->interpretation_payload, 'output_type_candidates.0.score');

        return is_numeric($candidateScore) ? round((float) $candidateScore, 4) : 0.0;
    }
}