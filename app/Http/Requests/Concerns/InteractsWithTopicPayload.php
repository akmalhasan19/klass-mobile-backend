<?php

namespace App\Http\Requests\Concerns;

use App\Models\SubSubject;
use Illuminate\Support\Arr;
use Illuminate\Validation\Validator;

trait InteractsWithTopicPayload
{
    protected function prepareTopicPayloadForValidation(): void
    {
        $subSubjectId = $this->input('sub_subject_id');
        $subjectId = $this->input('subject_id');
        $teacherId = $this->input('teacher_id');
        $thumbnailUrl = $this->input('thumbnail_url');

        if ($subSubjectId === null) {
            $subSubjectId = $this->input('taxonomy.sub_subject.id')
                ?? $this->input('taxonomy.sub_subject_id');
        }

        if ($subjectId === null) {
            $subjectId = $this->input('taxonomy.subject.id')
                ?? $this->input('taxonomy.subject_id');
        }

        if (! is_string($thumbnailUrl) || trim($thumbnailUrl) === '') {
            foreach (['media_url', 'image', 'imagePath'] as $legacyImageField) {
                $candidate = $this->input($legacyImageField);

                if (is_string($candidate) && filter_var($candidate, FILTER_VALIDATE_URL)) {
                    $thumbnailUrl = $candidate;
                    break;
                }
            }
        }

        $merge = [];

        if ($subSubjectId !== null && $subSubjectId !== '') {
            $merge['sub_subject_id'] = $subSubjectId;
        }

        if ($subjectId !== null && $subjectId !== '') {
            $merge['subject_id'] = $subjectId;
        }

        if ($teacherId !== null && $teacherId !== '') {
            $merge['teacher_id'] = (string) $teacherId;
        }

        if (is_string($thumbnailUrl) && trim($thumbnailUrl) !== '') {
            $merge['thumbnail_url'] = $thumbnailUrl;
        }

        if ($merge !== []) {
            $this->merge($merge);
        }
    }

    protected function addTopicTaxonomyConsistencyValidation(Validator $validator): void
    {
        $validator->after(function (Validator $validator): void {
            if ($validator->errors()->isNotEmpty()) {
                return;
            }

            $subSubjectId = $this->input('sub_subject_id');
            $subjectId = $this->input('subject_id');

            if ($subSubjectId === null || $subSubjectId === '' || $subjectId === null || $subjectId === '') {
                return;
            }

            $resolvedSubjectId = SubSubject::query()
                ->whereKey($subSubjectId)
                ->value('subject_id');

            if ($resolvedSubjectId === null || (int) $resolvedSubjectId === (int) $subjectId) {
                return;
            }

            $validator->errors()->add(
                'sub_subject_id',
                'Sub-subject yang dipilih tidak berada di dalam subject yang diberikan.'
            );
        });
    }

    /**
     * @return array<string, mixed>
     */
    public function topicAttributes(): array
    {
        return Arr::only($this->validated(), [
            'title',
            'teacher_id',
            'thumbnail_url',
            'sub_subject_id',
        ]);
    }
}