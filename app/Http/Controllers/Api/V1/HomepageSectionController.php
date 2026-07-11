<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Models\HomepageSection;
use Illuminate\Http\JsonResponse;

class HomepageSectionController extends Controller
{
    /**
     * Get configured homepage sections for the mobile app.
     */
    public function index(): JsonResponse
    {
        $sections = HomepageSection::where('is_enabled', true)
            ->orderBy('position', 'asc')
            ->get();

        return response()->json([
            'data' => $sections,
        ]);
    }
}
