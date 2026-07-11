<?php

namespace App\Http\Requests;

use App\Http\Requests\Concerns\InteractsWithTopicPayload;
use Illuminate\Validation\Validator;

class StoreTopicRequest extends ApiFormRequest
{
    use InteractsWithTopicPayload;

    protected function prepareForValidation(): void
    {
        $this->prepareTopicPayloadForValidation();
    }

    public function rules(): array
    {
        return [
            'title' => 'required|string|max:255',
            'teacher_id' => 'nullable|string|max:255',
            'sub_subject_id' => 'nullable|integer|exists:sub_subjects,id',
            'subject_id' => 'nullable|integer|exists:subjects,id',
            'taxonomy' => 'sometimes|array',
            'taxonomy.subject' => 'sometimes|array',
            'taxonomy.subject.id' => 'sometimes|integer|exists:subjects,id',
            'taxonomy.sub_subject' => 'sometimes|array',
            'taxonomy.sub_subject.id' => 'sometimes|integer|exists:sub_subjects,id',
            'thumbnail_url' => 'nullable|string|url|max:2048',
        ];
    }

    public function withValidator(Validator $validator): void
    {
        $this->addTopicTaxonomyConsistencyValidation($validator);
    }

    public function messages(): array
    {
        return [
            'title.required' => 'Judul topik wajib diisi.',
            'title.max' => 'Judul topik maksimal 255 karakter.',
            'sub_subject_id.exists' => 'Sub-subject yang dipilih tidak ditemukan.',
            'subject_id.exists' => 'Subject yang dipilih tidak ditemukan.',
            'thumbnail_url.url' => 'URL thumbnail harus berupa URL yang valid.',
        ];
    }
}
