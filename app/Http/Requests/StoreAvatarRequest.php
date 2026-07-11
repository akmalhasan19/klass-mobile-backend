<?php

namespace App\Http\Requests;

class StoreAvatarRequest extends ApiFormRequest
{
    public function rules(): array
    {
        return [
            'file' => 'required|file|mimes:jpg,jpeg,png,webp|max:2048',
        ];
    }

    public function messages(): array
    {
        return [
            'file.required' => 'File avatar wajib dikirim.',
            'file.file' => 'Input harus berupa file yang valid.',
            'file.mimes' => 'File avatar harus berupa gambar (jpg, jpeg, png, webp).',
            'file.max' => 'Ukuran file avatar maksimal 2 MB.',
        ];
    }
}
