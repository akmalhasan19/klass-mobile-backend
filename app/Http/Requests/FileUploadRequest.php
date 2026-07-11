<?php

namespace App\Http\Requests;

/**
 * FileUploadRequest
 *
 * Validasi request upload file.
 * Rules dinamis berdasarkan kategori upload yang dikirim di route parameter.
 */
class FileUploadRequest extends ApiFormRequest
{
    /**
     * Get the validation rules that apply to the request.
     *
     * @return array<string, \Illuminate\Contracts\Validation\ValidationRule|array<mixed>|string>
     */
    public function rules(): array
    {
        $category = $this->route('category');
        $config = config("filesystems.upload_categories.{$category}");

        if (!$config) {
            // Jika kategori tidak valid, biarkan controller/service handle error-nya
            return [
                'file' => ['required', 'file'],
            ];
        }

        $mimes = implode(',', $config['allowed_mimes']);
        $maxSize = $config['max_size_kb'];

        return [
            'file' => [
                'required',
                'file',
                "mimes:{$mimes}",
                "max:{$maxSize}",
            ],
        ];
    }

    /**
     * Custom validation messages.
     *
     * @return array<string, string>
     */
    public function messages(): array
    {
        $category = $this->route('category');
        $config = config("filesystems.upload_categories.{$category}");

        $maxMb = $config ? round($config['max_size_kb'] / 1024, 1) : '?';
        $mimes = $config ? implode(', ', $config['allowed_mimes']) : '?';

        return [
            'file.required' => 'File wajib dikirim.',
            'file.file' => 'Input harus berupa file yang valid.',
            'file.mimes' => "Tipe file tidak diizinkan untuk kategori '{$category}'. Tipe yang diizinkan: {$mimes}.",
            'file.max' => "Ukuran file melebihi batas maksimal {$maxMb} MB untuk kategori '{$category}'.",
        ];
    }
}
