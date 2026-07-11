<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class RegenerateMediaGenerationRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true; // Authorisasi ditangani di Controller (requireTeacher)
    }

    public function rules(): array
    {
        return [
            'additional_prompt' => ['required', 'string', 'max:5000'],
        ];
    }

    public function messages(): array
    {
        return [
            'additional_prompt.required' => 'Prompt tambahan wajib diisi untuk melakukan regenerasi.',
            'additional_prompt.max' => 'Prompt tambahan tidak boleh lebih dari 5000 karakter.',
        ];
    }
}
