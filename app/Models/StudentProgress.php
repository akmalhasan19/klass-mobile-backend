<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class StudentProgress extends Model
{
    use HasFactory, HasUuids;

    protected $table = 'student_progress';

    protected $fillable = [
        'student_name',
        'score',
        'completion_date',
    ];

    protected function casts(): array
    {
        return [
            'completion_date' => 'datetime',
        ];
    }
}
