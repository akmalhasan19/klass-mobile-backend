<?php

namespace App\Http\Resources;

use App\Models\SubSubject;
use App\Models\Subject;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class TopicResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        $subSubject = $this->subSubject;
        $subject = $subSubject?->subject;

        return [
            'id' => $this->id,
            'title' => $this->title,
            'teacher_id' => $this->teacher_id,
            'owner_user_id' => $this->owner_user_id,
            'ownership_status' => $this->ownership_status,
            'sub_subject_id' => $this->sub_subject_id,
            'subject_id' => $subject?->id ?? $subSubject?->subject_id,
            'taxonomy' => $this->serializeTaxonomy($subject, $subSubject),
            'personalization' => $this->resource->resolvePersonalizationContext(),
            'thumbnail_url' => $this->thumbnail_url,
            'is_published' => $this->is_published,
            'order' => $this->order,
            'contents_count' => $this->whenCounted('contents'),
            'contents' => ContentResource::collection($this->whenLoaded('contents')),
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
}
