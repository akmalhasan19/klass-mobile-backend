<?php

namespace App\Http\Requests;

class UpdateContentRequest extends ApiFormRequest
{
    public function rules(): array
    {
        return [
            'topic_id' => 'sometimes|uuid|exists:topics,id',
            'type' => 'sometimes|in:module,quiz,brief',
            'title' => 'nullable|string|max:255',
            'data' => 'nullable|array',
            'media_url' => 'nullable|string|url|max:2048',
        ];
    }

    public function messages(): array
    {
        return [
            'topic_id.uuid' => 'ID topik harus berupa UUID yang valid.',
            'topic_id.exists' => 'Topik tidak ditemukan.',
            'type.in' => 'Tipe konten harus salah satu dari: module, quiz, brief.',
            'media_url.url' => 'URL media harus berupa URL yang valid.',
        ];
    }
}
