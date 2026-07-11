<?php

namespace Database\Factories;

use App\Models\RecommendedProject;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<RecommendedProject>
 */
class RecommendedProjectFactory extends Factory
{
    protected $model = RecommendedProject::class;

    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'title' => fake()->sentence(3),
            'description' => fake()->paragraph(),
            'thumbnail_url' => fake()->imageUrl(1280, 720),
            'ratio' => '16:9',
            'project_type' => fake()->randomElement(['mobile', 'web', 'ui_ux']),
            'tags' => fake()->randomElements(['Flutter', 'Laravel', 'React', 'PostgreSQL'], fake()->numberBetween(1, 3)),
            'modules' => fake()->randomElements(['Auth', 'Dashboard', 'Inventory', 'Reports'], fake()->numberBetween(1, 3)),
            'source_type' => RecommendedProject::SOURCE_ADMIN_UPLOAD,
            'source_reference' => null,
            'source_payload' => null,
            'display_priority' => fake()->numberBetween(0, 100),
            'is_active' => true,
            'starts_at' => null,
            'ends_at' => null,
            'created_by' => null,
            'updated_by' => null,
        ];
    }

    public function inactive(): static
    {
        return $this->state(fn () => [
            'is_active' => false,
        ]);
    }

    public function scheduled(): static
    {
        return $this->state(fn () => [
            'starts_at' => now()->addDay(),
        ]);
    }

    public function expired(): static
    {
        return $this->state(fn () => [
            'ends_at' => now()->subDay(),
        ]);
    }
}