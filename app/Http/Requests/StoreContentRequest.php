<?php

namespace App\Http\Requests;

class StoreContentRequest extends ApiFormRequest
{
    public function rules(): array
    {
        return [
            'topic_id' => 'required|uuid|exists:topics,id',
            'type' => 'required|in:module,quiz,brief',
            'title' => 'nullable|string|max:255',
            'data' => 'nullable|array',
            'media_url' => 'nullable|string|url|max:2048',
        ];
    }

    public function messages(): array
    {
        return [
            'topic_id.required' => 'ID topik wajib diisi.',
            'topic_id.uuid' => 'ID topik harus berupa UUID yang valid.',
            'topic_id.exists' => 'Topik tidak ditemukan.',
            'type.required' => 'Tipe konten wajib diisi.',
            'type.in' => 'Tipe konten harus salah satu dari: module, quiz, brief.',
            'title.max' => 'Judul konten maksimal 255 karakter.',
            'media_url.url' => 'URL media harus berupa URL yang valid.',
        ];
    }
}
