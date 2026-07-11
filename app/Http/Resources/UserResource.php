<?php

namespace App\Http\Resources;

use App\Models\Subject;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class UserResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        $primarySubject = $this->primary_subject_id !== null ? $this->primarySubject : null;

        return [
            'id' => $this->id,
            'name' => $this->name,
            'email' => $this->email,
            'avatar_url' => $this->avatar_url,
            'primary_subject_id' => $this->primary_subject_id,
            'primary_subject' => $this->serializeSubject($primarySubject),
            'role' => $this->role,
            'is_admin' => $this->isAdmin(),
            'is_teacher' => $this->isTeacher(),
            'is_freelancer' => $this->isFreelancer(),
            'personalization_subject' => $this->when(
                $request->boolean('include_personalization_context'),
                function (): ?array {
                    $subject = $this->resolvePersonalizationSubjectAnchor();

                    if (! $subject) {
                        return null;
                    }

                    return array_merge($this->serializeSubject($subject) ?? [], [
                        'source' => $this->resolvePersonalizationSubjectSource(),
                    ]);
                }
            ),
            'email_verified_at' => $this->email_verified_at?->toISOString(),
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
}
