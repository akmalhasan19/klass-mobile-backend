<?php

namespace Database\Factories;

use App\Models\SubSubject;
use App\Models\Subject;
use Illuminate\Database\Eloquent\Factories\Factory;
use Illuminate\Support\Str;

/**
 * @extends Factory<SubSubject>
 */
class SubSubjectFactory extends Factory
{
    protected $model = SubSubject::class;

    public function definition(): array
    {
        $name = Str::title(fake()->unique()->words(fake()->numberBetween(1, 3), true));

        return [
            'subject_id' => Subject::factory(),
            'name' => $name,
            'slug' => Str::slug($name),
            'description' => fake()->sentence(),
            'display_order' => fake()->numberBetween(1, 20),
            'is_active' => true,
        ];
    }
}