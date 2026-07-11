<?php

namespace App\Http\Resources;

use App\MediaGeneration\MediaDraftTaxonomyHint;
use App\Models\SubSubject;
use App\Models\Subject;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class MediaGenerationTaxonomyDebugResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        $subSubject = $this->subSubject;
        $subject = $subSubject?->subject ?? $this->subject;
        $draftTaxonomyHint = is_array(data_get($this->decision_payload, 'content_draft.taxonomy_hint'))
            ? data_get($this->decision_payload, 'content_draft.taxonomy_hint')
            : MediaDraftTaxonomyHint::fromGeneration($this->resource);

        return [
            'id' => $this->id,
            'status' => (string) $this->status,
            'prompt' => $this->raw_prompt,
            'persisted_taxonomy' => [
                'subject' => $this->serializeSubject($subject),
                'sub_subject' => $this->serializeSubSubject($subSubject),
            ],
            'interpretation_context' => [
                'subject_context' => data_get($this->interpretation_payload, 'subject_context'),
                'sub_subject_context' => data_get($this->interpretation_payload, 'sub_subject_context'),
            ],
            'taxonomy_inference' => is_array(data_get($this->interpretation_audit_payload, 'taxonomy_inference'))
                ? data_get($this->interpretation_audit_payload, 'taxonomy_inference')
                : null,
            'draft_taxonomy_hint' => $draftTaxonomyHint,
            'drafting' => [
                'source' => data_get($this->decision_payload, 'content_draft.source'),
                'schema_version' => data_get($this->decision_payload, 'content_draft.schema_version'),
                'fallback_triggered' => (bool) data_get($this->decision_payload, 'content_draft.draft_fallback_triggered', false),
                'fallback_reason_code' => data_get($this->decision_payload, 'content_draft.draft_fallback_reason_code'),
            ],
            'links' => [
                'poll' => url('/api/v1/media-generations/' . $this->id),
            ],
            'created_at' => $this->created_at?->toISOString(),
            'updated_at' => $this->updated_at?->toISOString(),
        ];
    }

    protected function serializeSubject(?Subject $subject): ?array
    {
        if (! $subject) {
            return null;
        }

        return [
            'id' => $subject->id,
            'name' => $subject->name,
            'slug' => $subject->slug,
        ];
    }

    protected function serializeSubSubject(?SubSubject $subSubject): ?array
    {
        if (! $subSubject) {
            return null;
        }

        return [
            'id' => $subSubject->id,
            'subject_id' => $subSubject->subject_id,
            'name' => $subSubject->name,
            'slug' => $subSubject->slug,
        ];
    }
}