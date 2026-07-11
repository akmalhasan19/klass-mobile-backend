<?php

namespace Tests\Unit\Services;

use App\Models\MediaGeneration;
use App\Models\Subject;
use App\Models\SubSubject;
use App\Models\User;
use App\Services\FreelancerMatchingService;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Collection;
use Tests\TestCase;

class FreelancerMatchingServiceTest extends TestCase
{
    use RefreshDatabase;

    private FreelancerMatchingService $service;

    protected function setUp(): void
    {
        parent::setUp();
        $this->service = new FreelancerMatchingService();
    }

    public function test_match_by_portfolio_returns_scores(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
        ]);
        
        $freelancers = User::factory()->count(3)->create(['role' => User::ROLE_FREELANCER]);

        $scores = $this->service->matchByPortfolio($generation, $freelancers);

        $this->assertCount(3, $scores);
        foreach ($freelancers as $freelancer) {
            $this->assertArrayHasKey($freelancer->id, $scores);
            $this->assertGreaterThanOrEqual(0.0, $scores[$freelancer->id]);
            $this->assertLessThanOrEqual(1.0, $scores[$freelancer->id]);
        }
    }

    public function test_match_by_success_rate_returns_scores(): void
    {
        $freelancers = User::factory()->count(2)->create(['role' => User::ROLE_FREELANCER]);

        $scores = $this->service->matchBySuccessRate($freelancers);

        $this->assertCount(2, $scores);
        foreach ($freelancers as $freelancer) {
            $this->assertArrayHasKey($freelancer->id, $scores);
            $this->assertGreaterThanOrEqual(0.0, $scores[$freelancer->id]);
            $this->assertLessThanOrEqual(1.0, $scores[$freelancer->id]);
        }
    }

    public function test_match_by_availability_returns_scores(): void
    {
        $freelancers = User::factory()->count(4)->create(['role' => User::ROLE_FREELANCER]);

        $scores = $this->service->matchByAvailability($freelancers);

        $this->assertCount(4, $scores);
        foreach ($freelancers as $freelancer) {
            $this->assertArrayHasKey($freelancer->id, $scores);
            $this->assertGreaterThanOrEqual(0.0, $scores[$freelancer->id]);
            $this->assertLessThanOrEqual(1.0, $scores[$freelancer->id]);
        }
    }

    public function test_find_best_matches_returns_empty_when_no_freelancers(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
        ]);

        $matches = $this->service->findBestMatches($generation);

        $this->assertInstanceOf(Collection::class, $matches);
        $this->assertTrue($matches->isEmpty());
    }

    public function test_find_best_matches_calculates_weighted_scores_and_limits_results(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);
        $teacher = User::factory()->teacher()->create();
        $subSubject = SubSubject::query()->where('slug', 'algebra')->firstOrFail();
        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'subject_id' => $subSubject->subject_id,
            'sub_subject_id' => $subSubject->id,
            'raw_prompt' => 'Buatkan handout.',
            'preferred_output_type' => 'pdf',
        ]);
        
        User::factory()->count(10)->create(['role' => User::ROLE_FREELANCER]);

        $matches = $this->service->findBestMatches($generation, 3);

        $this->assertCount(3, $matches);

        $previousScore = 1.1; // Max possible is 1.0
        foreach ($matches as $match) {
            $this->assertArrayHasKey('freelancer', $match);
            $this->assertArrayHasKey('match_score', $match);
            
            $score = $match['match_score'];
            $this->assertLessThanOrEqual($previousScore, $score, 'Results must be sorted descending');
            $previousScore = $score;
            
            $expectedScore = ($match['portfolio_relevance_score'] * 0.5) 
                           + ($match['success_rate'] * 0.3) 
                           + ($match['availability_score'] * 0.2);
            $this->assertEqualsWithDelta($expectedScore, $score, 0.0001);
        }
    }
}
