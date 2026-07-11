<?php

namespace App\Services;

use App\Models\MediaGeneration;
use App\Models\User;
use Illuminate\Support\Collection;

class FreelancerMatchingService
{
    /**
     * Finds the best N matches for a given generation.
     */
    public function findBestMatches(MediaGeneration $generation, int $limit = 5): Collection
    {
        $candidates = User::query()
            ->where('role', User::ROLE_FREELANCER)
            ->get();

        if ($candidates->isEmpty()) {
            return collect();
        }

        $portfolioScores = $this->matchByPortfolio($generation, $candidates);
        $successRateScores = $this->matchBySuccessRate($candidates);
        $availabilityScores = $this->matchByAvailability($candidates);

        $results = collect();

        foreach ($candidates as $candidate) {
            $pScore = $portfolioScores[$candidate->id] ?? 0.0;
            $sScore = $successRateScores[$candidate->id] ?? 0.0;
            $aScore = $availabilityScores[$candidate->id] ?? 0.0;

            // Algorithm weights: Portfolio 50%, Success Rate 30%, Availability 20%
            $totalScore = ($pScore * 0.5) + ($sScore * 0.3) + ($aScore * 0.2);

            $results->push([
                'freelancer' => $candidate,
                'portfolio_relevance_score' => $pScore,
                'success_rate' => $sScore,
                'availability_score' => $aScore,
                'match_score' => $totalScore,
            ]);
        }

        return $results->sortByDesc('match_score')->take($limit)->values();
    }

    /**
     * Compute portfolio relevance score.
     * Stubbed to return a random/derived float between 0.0 and 1.0 until actual portfolio data exists.
     *
     * @return array<int, float> Mapping of freelancer ID to score
     */
    public function matchByPortfolio(MediaGeneration $generation, Collection $candidates): array
    {
        $scores = [];
        $type = $generation->resolved_output_type ?? $generation->preferred_output_type;
        $hashBase = md5($type . $generation->subject_id);

        foreach ($candidates as $candidate) {
            // Predictably generate a score between 0.4 and 1.0 based on freelancer ID and context
            $derivedScore = (hexdec(substr(md5($hashBase . $candidate->id), 0, 8)) / 4294967295);
            $scores[$candidate->id] = 0.4 + ($derivedScore * 0.6);
        }

        return $scores;
    }

    /**
     * Compute success rate score.
     * Stubbed to simulate highly rated freelancers.
     *
     * @return array<int, float> Mapping of freelancer ID to score
     */
    public function matchBySuccessRate(Collection $candidates): array
    {
        $scores = [];

        foreach ($candidates as $candidate) {
            // Predictably generate a success rate between 0.7 (70%) and 1.0 (100%)
            $derivedScore = (hexdec(substr(md5('success' . $candidate->id), 0, 8)) / 4294967295);
            $scores[$candidate->id] = 0.7 + ($derivedScore * 0.3);
        }

        return $scores;
    }

    /**
     * Compute availability score.
     * Stubbed to simulate active freelancers.
     *
     * @return array<int, float> Mapping of freelancer ID to score
     */
    public function matchByAvailability(Collection $candidates): array
    {
        $scores = [];

        foreach ($candidates as $candidate) {
            // Predictably generate availability between 0.5 and 1.0
            $derivedScore = (hexdec(substr(md5('avail' . $candidate->id), 0, 8)) / 4294967295);
            $scores[$candidate->id] = 0.5 + ($derivedScore * 0.5);
        }

        return $scores;
    }
}
