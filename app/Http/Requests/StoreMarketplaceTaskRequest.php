<?php

namespace App\Http\Requests;

class StoreMarketplaceTaskRequest extends ApiFormRequest
{
    public function rules(): array
    {
        return [
            'content_id' => 'required|uuid|exists:contents,id',
            'status' => 'sometimes|in:open,taken,done',
            'creator_id' => 'nullable|string|max:255',
            'attachment_url' => 'nullable|string|url|max:2048',
        ];
    }

    public function messages(): array
    {
        return [
            'content_id.required' => 'ID konten wajib diisi.',
            'content_id.uuid' => 'ID konten harus berupa UUID yang valid.',
            'content_id.exists' => 'Konten tidak ditemukan.',
            'status.in' => 'Status harus salah satu dari: open, taken, done.',
            'attachment_url.url' => 'URL lampiran harus berupa URL yang valid.',
        ];
    }
}
