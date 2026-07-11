<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Resources\MediaGenerationTaxonomyDebugResource;
use App\Http\Traits\ApiResponseTrait;
use App\Models\MediaGeneration;
use Illuminate\Http\JsonResponse;

class AdminMediaGenerationDebugController extends Controller
{
    use ApiResponseTrait;

    public function show(MediaGeneration $mediaGeneration): JsonResponse
    {
        $mediaGeneration->loadMissing(['subject', 'subSubject.subject']);

        return $this->success(
            new MediaGenerationTaxonomyDebugResource($mediaGeneration),
            'Debug taxonomy media generation berhasil diambil.'
        );
    }
}