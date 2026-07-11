<?php

namespace App\Http\Requests;

use App\MediaGeneration\MediaGenerationErrorCode;
use App\Models\MediaGeneration;
use App\Models\SubSubject;
use Illuminate\Contracts\Validation\Validator;
use Illuminate\Http\Exceptions\HttpResponseException;

class StoreMediaGenerationRequest extends ApiFormRequest
{
    protected function prepareForValidation(): void
    {
        $prompt = $this->input('prompt');
        $preferredOutputType = $this->input('preferred_output_type');

        $merge = [];

        if (is_string($prompt)) {
            $merge['prompt'] = trim($prompt);
        }

        if (is_string($preferredOutputType) && trim($preferredOutputType) !== '') {
            $merge['preferred_output_type'] = strtolower(trim($preferredOutputType));
        }

        if ($merge !== []) {
            $this->merge($merge);
        }
    }

    public function rules(): array
    {
        return [
            'prompt' => 'required|string|max:5000',
            'preferred_output_type' => 'nullable|string|in:auto,docx,pdf,pptx',
            'subject_id' => 'nullable|integer|exists:subjects,id',
            'sub_subject_id' => 'nullable|integer|exists:sub_subjects,id',
        ];
    }

    public function withValidator($validator): void
    {
        $validator->after(function ($validator): void {
            $subjectId = $this->input('subject_id');
            $subSubjectId = $this->input('sub_subject_id');

            if ($subjectId === null || $subSubjectId === null) {
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

    public function messages(): array
    {
        return [
            'prompt.required' => 'Prompt media generation wajib diisi.',
            'prompt.max' => 'Prompt media generation maksimal 5000 karakter.',
            'preferred_output_type.in' => 'Preferred output type harus salah satu dari: auto, docx, pdf, atau pptx.',
            'subject_id.exists' => 'Subject yang dipilih tidak ditemukan.',
            'sub_subject_id.exists' => 'Sub-subject yang dipilih tidak ditemukan.',
        ];
    }

    /**
     * @return array{prompt: string, preferred_output_type: string, subject_id: int|null, sub_subject_id: int|null}
     */
    public function generationAttributes(): array
    {
        $validated = $this->validated();
        $subSubjectId = isset($validated['sub_subject_id']) ? (int) $validated['sub_subject_id'] : null;
        $subjectId = isset($validated['subject_id']) ? (int) $validated['subject_id'] : null;

        if ($subjectId === null && $subSubjectId !== null) {
            $subjectId = SubSubject::query()
                ->whereKey($subSubjectId)
                ->value('subject_id');
        }

        return [
            'prompt' => trim((string) $validated['prompt']),
            'preferred_output_type' => MediaGeneration::normalizePreferredOutputType($validated['preferred_output_type'] ?? null),
            'subject_id' => $subjectId,
            'sub_subject_id' => $subSubjectId,
        ];
    }

    protected function failedValidation(Validator $validator): void
    {
        throw new HttpResponseException(
            response()->json([
                'success' => false,
                'message' => 'Validasi gagal.',
                'error' => MediaGenerationErrorCode::toClientPayload(MediaGenerationErrorCode::VALIDATION_FAILED),
                'errors' => $validator->errors(),
            ], 422)
        );
    }
}