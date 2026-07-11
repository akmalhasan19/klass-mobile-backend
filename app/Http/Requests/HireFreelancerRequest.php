<?php

namespace App\Http\Requests;

use App\Models\MarketplaceTask;
use Illuminate\Foundation\Http\FormRequest;

class HireFreelancerRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true; // Authorisasi ditangani di controller (requireTeacher)
    }

    public function rules(): array
    {
        return [
            'mode' => ['required', 'string', 'in:auto_suggest,manual_task'],
            'refinement_description' => ['required', 'string', 'max:2000'],
            'selected_freelancer_id' => ['required_if:mode,auto_suggest', 'integer', 'exists:users,id'],
        ];
    }

    public function messages(): array
    {
        return [
            'mode.required' => 'Mode penyewaan wajib dipilih.',
            'mode.in' => 'Mode penyewaan tidak valid.',
            'refinement_description.required' => 'Deskripsi perbaikan wajib diisi.',
            'refinement_description.max' => 'Deskripsi perbaikan maksimal 2000 karakter.',
            'selected_freelancer_id.required_if' => 'Freelancer wajib dipilih jika menggunakan mode pencarian otomatis.',
            'selected_freelancer_id.exists' => 'Freelancer yang dipilih tidak valid atau tidak ditemukan.',
        ];
    }
}
