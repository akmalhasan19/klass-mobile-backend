<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;

class HomepageSection extends Model
{
    use HasUuids;

    protected $fillable = [
        'key',
        'label',
        'position',
        'is_enabled',
        'data_source',
    ];

    protected function casts(): array
    {
        return [
            'is_enabled' => 'boolean',
            'position' => 'integer',
        ];
    }
}
