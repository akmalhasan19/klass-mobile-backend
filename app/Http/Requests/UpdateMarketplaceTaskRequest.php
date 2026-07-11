<?php

namespace App\Http\Requests;

class UpdateMarketplaceTaskRequest extends ApiFormRequest
{
    public function rules(): array
    {
        return [
            'status' => 'sometimes|in:open,taken,done',
            'creator_id' => 'nullable|string|max:255',
            'attachment_url' => 'nullable|string|url|max:2048',
        ];
    }

    public function messages(): array
    {
        return [
            'status.in' => 'Status harus salah satu dari: open, taken, done.',
            'attachment_url.url' => 'URL lampiran harus berupa URL yang valid.',
        ];
    }
}
