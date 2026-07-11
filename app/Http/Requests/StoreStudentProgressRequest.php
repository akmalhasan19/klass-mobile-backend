<?php

namespace App\Http\Requests;

class StoreStudentProgressRequest extends ApiFormRequest
{
    public function rules(): array
    {
        return [
            'student_name' => 'required|string|max:255',
            'score' => 'required|integer|min:0|max:100',
            'completion_date' => 'nullable|date',
        ];
    }

    public function messages(): array
    {
        return [
            'student_name.required' => 'Nama siswa wajib diisi.',
            'student_name.max' => 'Nama siswa maksimal 255 karakter.',
            'score.required' => 'Skor wajib diisi.',
            'score.integer' => 'Skor harus berupa angka bulat.',
            'score.min' => 'Skor minimal 0.',
            'score.max' => 'Skor maksimal 100.',
            'completion_date.date' => 'Tanggal selesai harus berupa tanggal yang valid.',
        ];
    }
}
