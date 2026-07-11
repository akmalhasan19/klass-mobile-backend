<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Traits\ApiResponseTrait;
use App\MediaGeneration\MediaGenerationApiException;
use App\Models\FreelancerMatch;
use App\Models\MediaGeneration;
use App\Services\FreelancerMatchingService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class FreelancerSuggestionController extends Controller
{
    use ApiResponseTrait;

    public function suggest(
        Request $request,
        FreelancerMatchingService $matchingService,
        string $mediaGeneration
    ): JsonResponse {
        $teacher = $request->user();

        if (! $teacher || ! $teacher->isTeacher()) {
            throw MediaGenerationApiException::teacherRoleRequired();
        }

        $generation = MediaGeneration::query()
            ->whereKey($mediaGeneration)
            ->where('teacher_id', $teacher->id)
            ->first();

        if (! $generation) {
            throw MediaGenerationApiException::notFound();
        }

        if (! $generation->isTerminal()) {
            return $this->error(
                'Media generation belum selesai. Tidak dapat mencari freelancer untuk task yang masih diproses.',
                422
            );
        }

        $limit = (int) $request->input('max_suggestions', 5);
        $limit = min(max($limit, 1), 10); // clamp between 1 and 10

        $bestMatches = $matchingService->findBestMatches($generation, $limit);

        // Store or update matches in DB for auditing
        foreach ($bestMatches as $matchData) {
            FreelancerMatch::updateOrCreate(
                [
                    'media_generation_id' => $generation->id,
                    'freelancer_id' => $matchData['freelancer']->id,
                ],
                [
                    'match_score' => $matchData['match_score'],
                    'portfolio_relevance_score' => $matchData['portfolio_relevance_score'],
                    'success_rate' => $matchData['success_rate'],
                ]
            );
        }

        $transformedMatches = $bestMatches->map(function ($matchData) {
            $freelancer = $matchData['freelancer'];
            return [
                'freelancer' => [
                    'id' => $freelancer->id,
                    'name' => $freelancer->name,
                    'rating' => 4.8, // Stubbed metric
                ],
                'match_score' => $matchData['match_score'],
                'portfolio_relevance_score' => $matchData['portfolio_relevance_score'],
                'success_rate' => $matchData['success_rate'],
            ];
        });

        return $this->success(
            $transformedMatches,
            'Saran freelancer berhasil didapatkan.'
        );
    }
}
