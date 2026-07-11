<?php

namespace Database\Seeders;

use App\MediaGeneration\SubjectsJsonTaxonomyCatalog;
use App\Models\SubSubject;
use App\Models\Subject;
use Illuminate\Database\Seeder;

class SubjectTaxonomySeeder extends Seeder
{
    public function run(): void
    {
        $taxonomy = array_merge(
            $this->legacyTaxonomy(),
            $this->subjectsJsonTaxonomy(),
        );

        foreach ($taxonomy as $subjectIndex => $subjectData) {
            $subject = Subject::updateOrCreate(
                ['slug' => $subjectData['slug']],
                [
                    'name' => $subjectData['name'],
                    'description' => $subjectData['description'],
                    'display_order' => $subjectIndex + 1,
                    'is_active' => $subjectData['is_active'] ?? true,
                ],
            );

            foreach ($subjectData['sub_subjects'] as $subSubjectIndex => $subSubjectData) {
                SubSubject::updateOrCreate(
                    [
                        'subject_id' => $subject->id,
                        'slug' => $subSubjectData['slug'],
                    ],
                    [
                        'name' => $subSubjectData['name'],
                        'description' => $subSubjectData['description'],
                        'display_order' => $subSubjectIndex + 1,
                        'is_active' => $subSubjectData['is_active'] ?? true,
                    ],
                );
            }
        }
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private function legacyTaxonomy(): array
    {
        return [
            [
                'name' => 'History',
                'slug' => 'history',
                'description' => 'Historical learning topics and civic context.',
                'is_active' => true,
                'sub_subjects' => [
                    ['name' => 'Indonesian History', 'slug' => 'indonesian-history', 'description' => 'National history, independence, and modern eras.', 'is_active' => true],
                    ['name' => 'World History', 'slug' => 'world-history', 'description' => 'Global civilizations, wars, and political change.', 'is_active' => true],
                    ['name' => 'Civics', 'slug' => 'civics', 'description' => 'Government, citizenship, and public institutions.', 'is_active' => true],
                ],
            ],
            [
                'name' => 'Health',
                'slug' => 'health',
                'description' => 'Nutrition, wellness, and personal health topics.',
                'is_active' => true,
                'sub_subjects' => [
                    ['name' => 'Nutrition', 'slug' => 'nutrition', 'description' => 'Balanced diet, macro, and micronutrients.', 'is_active' => true],
                    ['name' => 'Healthy Lifestyle', 'slug' => 'healthy-lifestyle', 'description' => 'Daily habits, sleep, and preventive care.', 'is_active' => true],
                    ['name' => 'Public Health', 'slug' => 'public-health', 'description' => 'Population-level health and safety topics.', 'is_active' => true],
                ],
            ],
            [
                'name' => 'Mathematics',
                'slug' => 'mathematics',
                'description' => 'Core numeracy, problem-solving, and mathematical reasoning.',
                'is_active' => true,
                'sub_subjects' => [
                    ['name' => 'Algebra', 'slug' => 'algebra', 'description' => 'Equations, variables, and algebraic reasoning.', 'is_active' => true],
                    ['name' => 'Geometry', 'slug' => 'geometry', 'description' => 'Shapes, angles, and spatial thinking.', 'is_active' => true],
                    ['name' => 'Arithmetic', 'slug' => 'arithmetic', 'description' => 'Foundational operations and number fluency.', 'is_active' => true],
                ],
            ],
            [
                'name' => 'Science',
                'slug' => 'science',
                'description' => 'Scientific foundations across physics and related disciplines.',
                'is_active' => true,
                'sub_subjects' => [
                    ['name' => 'Physics', 'slug' => 'physics', 'description' => 'Motion, forces, and physical systems.', 'is_active' => true],
                    ['name' => 'Thermodynamics', 'slug' => 'thermodynamics', 'description' => 'Energy, heat, and entropy.', 'is_active' => true],
                    ['name' => 'Quantum Physics', 'slug' => 'quantum-physics', 'description' => 'Wave behavior and modern quantum concepts.', 'is_active' => true],
                ],
            ],
            [
                'name' => 'Arts and Humanities',
                'slug' => 'arts-and-humanities',
                'description' => 'Art, culture, and creative interpretation topics.',
                'is_active' => true,
                'sub_subjects' => [
                    ['name' => 'Art History', 'slug' => 'art-history', 'description' => 'Movements, artists, and visual culture.', 'is_active' => true],
                    ['name' => 'Visual Design', 'slug' => 'visual-design', 'description' => 'Composition, imagery, and visual communication.', 'is_active' => true],
                    ['name' => 'Creative Studies', 'slug' => 'creative-studies', 'description' => 'Creative process and artistic exploration.', 'is_active' => true],
                ],
            ],
        ];
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private function subjectsJsonTaxonomy(): array
    {
        $taxonomy = [];

        foreach (SubjectsJsonTaxonomyCatalog::groupedSubjects() as $group) {
            $subSubjects = [];

            foreach ($group['entries'] as $entry) {
                $subSubjects[$entry['sub_subject_slug']] = [
                    'name' => $entry['sub_subject_name'],
                    'slug' => $entry['sub_subject_slug'],
                    'description' => $entry['description'],
                    'is_active' => $entry['is_active'],
                ];
            }

            $taxonomy[] = [
                'name' => $group['subject_name'],
                'slug' => $group['subject_slug'],
                'description' => $this->subjectsJsonDescription($group),
                'is_active' => $group['is_active'],
                'sub_subjects' => array_values($subSubjects),
            ];
        }

        return $taxonomy;
    }

    /**
     * @param  array<string, mixed>  $group
     */
    private function subjectsJsonDescription(array $group): string
    {
        $jenjang = trim((string) ($group['jenjang'] ?? ''));
        $subjectName = trim((string) ($group['subject_name'] ?? ''));
        $entries = (array) ($group['entries'] ?? []);
        $classes = array_values(array_unique(array_filter(array_map(
            static fn (mixed $entry): ?int => is_array($entry) && is_numeric($entry['kelas'] ?? null) ? (int) $entry['kelas'] : null,
            $entries
        ))));

        sort($classes);

        $classLabel = $classes === []
            ? ''
            : ' kelas ' . ($classes[0] === $classes[array_key_last($classes)]
                ? (string) $classes[0]
                : $classes[0] . '-' . $classes[array_key_last($classes)]);

        return trim('Taxonomy import untuk ' . $subjectName . ' jenjang ' . $jenjang . $classLabel . '.');
    }
}