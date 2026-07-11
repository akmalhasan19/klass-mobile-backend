<?php

namespace App\Services;

use App\Models\MediaGeneration;

class RegenerationContextService
{
    /**
     * Builds context info from a MediaGeneration for use by the Freelancer matcher, notifications, etc.
     */
    public function getRegenerationContext(MediaGeneration $generation): array
    {
        $generation->loadMissing(['subject', 'teacher', 'content']);

        return [
            'generation_id' => $generation->id,
            'is_regeneration' => $generation->isRegeneration(),
            'original_generation_id' => $generation->getOriginalGeneration()->id,
            'subject_name' => $generation->subject?->name ?? 'Unknown',
            'output_type' => $generation->resolved_output_type ?? $generation->preferred_output_type,
            'teacher_name' => $generation->teacher?->name,
            'content_title' => $generation->content?->title,
            'current_status' => $generation->status,
        ];
    }
}
