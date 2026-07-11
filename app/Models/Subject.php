<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Subject extends Model
{
    use HasFactory;

    protected $fillable = [
        'name',
        'slug',
        'description',
        'display_order',
        'is_active',
    ];

    protected function casts(): array
    {
        return [
            'display_order' => 'integer',
            'is_active' => 'boolean',
        ];
    }

    public function subSubjects(): HasMany
    {
        return $this->hasMany(SubSubject::class)
            ->orderBy('display_order')
            ->orderBy('name');
    }

    public function users(): HasMany
    {
        return $this->hasMany(User::class, 'primary_subject_id');
    }
}