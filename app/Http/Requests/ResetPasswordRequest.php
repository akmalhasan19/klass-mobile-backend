<?php

namespace App\Http\Requests;

class ResetPasswordRequest extends ApiFormRequest
{
    public function rules(): array
    {
        return [
            'email' => 'required|email',
            'security_answer' => 'required|string',
            'new_password' => 'required|string|min:6',
        ];
    }

    public function messages(): array
    {
        return [
            'email.required' => 'Email wajib diisi.',
            'email.email' => 'Format email tidak valid.',
            'security_answer.required' => 'Jawaban keamanan wajib diisi.',
            'new_password.required' => 'Password baru wajib diisi.',
            'new_password.min' => 'Password baru minimal 6 karakter.',
        ];
    }
}
