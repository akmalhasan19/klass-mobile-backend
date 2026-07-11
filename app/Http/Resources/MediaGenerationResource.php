<?php

namespace App\Http\Resources;

use App\MediaGeneration\MediaGenerationErrorCode;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\SubSubject;
use App\Models\Subject;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class MediaGenerationResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        $status = (string) $this->status;
        $knownStatus = in_array($status, MediaGenerationLifecycle::all(), true);
        $subSubject = $this->subSubject;
        $subject = $subSubject?->subject ?? $this->subject;

        return [
            'id' => $this->id,
            // Parent-chain tracking fields (RF-05: Generation History)
            'generated_from_id' => $this->generated_from_id,
            'is_regeneration' => (bool) $this->is_regeneration,
            'teacher_id' => $this->teacher_id,
            'prompt' => $this->raw_prompt,
            'preferred_output_type' => $this->preferred_output_type,
            'resolved_output_type' => $this->resolved_output_type,
            'status' => $status,
            'status_meta' => [
                'lifecycle_version' => MediaGenerationLifecycle::VERSION,
                'is_terminal' => $knownStatus ? MediaGenerationLifecycle::isTerminal($status) : false,
                'retry_behavior' => $knownStatus ? MediaGenerationLifecycle::retryBehavior($status) : null,
            ],
            'subject_id' => $subject?->id ?? $subSubject?->subject_id ?? $this->subject_id,
            'sub_subject_id' => $this->sub_subject_id,
            'taxonomy' => $this->serializeTaxonomy($subject, $subSubject),
            'provider' => [
                'llm' => [
                    'name' => $this->llm_provider,
                    'model' => $this->llm_model,
                ],
                'generator' => [
                    'name' => $this->generator_provider,
                    'model' => $this->generator_model,
                ],
            ],
            'artifact' => [
                'storage_path' => $this->storage_path,
                'file_url' => $this->file_url,
                'thumbnail_url' => $this->thumbnail_url,
                'mime_type' => $this->mime_type,
            ],
            'publication' => [
                'topic' => $this->topic_id ? [
                    'id' => $this->topic_id,
                    'title' => $this->topic?->title,
                ] : null,
                'content' => $this->content_id ? [
                    'id' => $this->content_id,
                    'title' => $this->content?->title,
                    'type' => $this->content?->type,
                    'media_url' => $this->content?->media_url,
                ] : null,
                'recommended_project' => $this->recommended_project_id ? [
                    'id' => (string) $this->recommended_project_id,
                    'title' => $this->recommendedProject?->title,
                    'source_type' => $this->recommendedProject?->source_type,
                    'project_file_url' => $this->recommendedProject?->project_file_url,
                ] : null,
            ],
            'delivery_payload' => $this->delivery_payload,
            'error' => $this->serializeError($status),
            'links' => [
                'poll' => url('/api/v1/media-generations/' . $this->id),
            ],
            'created_at' => $this->created_at?->toISOString(),
            'updated_at' => $this->updated_at?->toISOString(),
        ];
    }

    protected function serializeTaxonomy(?Subject $subject, ?SubSubject $subSubject): ?array
    {
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

    /**
     * @return array{code: string, message: string, retryable: bool}|null
     */
    protected function serializeError(string $status): ?array
    {
        $errorCode = $this->error_code;

        if ((! is_string($errorCode) || trim($errorCode) === '') && $status === MediaGenerationLifecycle::FAILED) {
            $errorCode = MediaGenerationErrorCode::PUBLICATION_FAILED;
        }

        return MediaGenerationErrorCode::toClientPayload($errorCode);
    }
}