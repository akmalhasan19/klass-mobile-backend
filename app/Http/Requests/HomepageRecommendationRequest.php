<?php

namespace App\Http\Requests;

class HomepageRecommendationRequest extends ApiFormRequest
{
    public function rules(): array
    {
        return [
            'limit' => 'nullable|integer|min:1|max:50',
        ];
    }

    public function messages(): array
    {
        return [
            'limit.integer' => 'Parameter limit harus berupa angka.',
            'limit.min' => 'Parameter limit minimal 1.',
            'limit.max' => 'Parameter limit maksimal 50.',
        ];
    }
}
