<?php

namespace Database\Seeders;

use App\Models\Content;
use App\Models\SubSubject;
use App\Models\Topic;
use App\Models\User;
use App\Services\FileUploadService;
use Illuminate\Database\Seeder;

class TopicAndContentSeeder extends Seeder
{
    public function run(): void
    {
        $uploadService = app(FileUploadService::class);
        $teacher = User::where('email', 'sarah.jenkins@klass.id')->first() ?? User::factory()->create();
        $subSubjects = SubSubject::query()
            ->whereIn('slug', [
                'indonesian-history',
                'nutrition',
                'algebra',
                'quantum-physics',
                'art-history',
                'thermodynamics',
            ])
            ->get()
            ->keyBy('slug');

        // 1. History Topic
        $topicHistory = Topic::updateOrCreate([
            'title' => 'Modern History of Indonesia',
            'teacher_id' => $teacher->id,
        ], [
            'thumbnail_url' => $uploadService->generatePublicUrl('gallery/ppt_design_3.jpg'),
            'sub_subject_id' => $subSubjects->get('indonesian-history')?->id,
        ]);

        $historyModules = [
            ['title' => 'Masa Kolonial Belanda', 'type' => 'module'],
            ['title' => 'Perjuangan Kemerdekaan', 'type' => 'module'],
            ['title' => 'Era Orde Lama & Baru', 'type' => 'module'],
            ['title' => 'Indonesia Modern', 'type' => 'module'],
        ];

        foreach ($historyModules as $module) {
            Content::firstOrCreate([
                'topic_id' => $topicHistory->id,
                'title' => $module['title'],
            ], [
                'type' => $module['type'],
                'data' => [],
                'media_url' => $uploadService->generatePublicUrl('gallery/ppt_design_3.jpg'),
            ]);
        }

        // 2. Health Topic
        $topicHealth = Topic::updateOrCreate([
            'title' => 'Benefits of Healthy Eating',
            'teacher_id' => $teacher->id,
        ], [
            'thumbnail_url' => $uploadService->generatePublicUrl('gallery/infographic_preview_health.png'),
            'sub_subject_id' => $subSubjects->get('nutrition')?->id,
        ]);

        $healthModules = [
            ['title' => 'Pentingnya Makronutrien', 'type' => 'brief'],
            ['title' => 'Mikronutrien Esensial', 'type' => 'brief'],
            ['title' => 'Dampak Jangka Panjang', 'type' => 'module'],
        ];

        foreach ($healthModules as $module) {
            Content::firstOrCreate([
                'topic_id' => $topicHealth->id,
                'title' => $module['title'],
            ], [
                'type' => $module['type'],
                'data' => [],
                'media_url' => $uploadService->generatePublicUrl('gallery/infographic_preview_health.png'),
            ]);
        }

        // 3. Math Topic
        $topicMath = Topic::updateOrCreate([
            'title' => 'Mathematics Quiz',
            'teacher_id' => $teacher->id,
        ], [
            'thumbnail_url' => $uploadService->generatePublicUrl('gallery/square_preview_math.png'),
            'sub_subject_id' => $subSubjects->get('algebra')?->id,
        ]);

        $mathModules = [
            ['title' => 'Aljabar Dasar', 'type' => 'quiz'],
            ['title' => 'Geometri', 'type' => 'quiz'],
            ['title' => 'Aritmatika Lanjut', 'type' => 'quiz'],
        ];

        foreach ($mathModules as $module) {
            Content::firstOrCreate([
                'topic_id' => $topicMath->id,
                'title' => $module['title'],
            ], [
                'type' => $module['type'],
                'data' => [],
                'media_url' => $uploadService->generatePublicUrl('gallery/square_preview_math.png'),
            ]);
        }

        // Profile Modules (Draft/Published mix)
        $topicPhysics = Topic::updateOrCreate([
            'title' => 'Intro to Quantum Physics',
            'teacher_id' => $teacher->id,
        ], [
            'thumbnail_url' => null,
            'sub_subject_id' => $subSubjects->get('quantum-physics')?->id,
        ]);
        Content::firstOrCreate(['topic_id' => $topicPhysics->id, 'title' => 'Wave-Particle Duality'], ['type' => 'module', 'media_url' => '']);
        
        $topicArt = Topic::updateOrCreate([
            'title' => 'Modern Art History',
            'teacher_id' => $teacher->id,
        ], [
            'thumbnail_url' => null,
            'sub_subject_id' => $subSubjects->get('art-history')?->id,
        ]);
        Content::firstOrCreate(['topic_id' => $topicArt->id, 'title' => 'Impressionism'], ['type' => 'module', 'media_url' => '']);

        $topicThermo = Topic::updateOrCreate([
            'title' => 'Advanced Thermodynamics',
            'teacher_id' => $teacher->id,
        ], [
            'thumbnail_url' => null,
            'sub_subject_id' => $subSubjects->get('thermodynamics')?->id,
        ]);
        Content::firstOrCreate(['topic_id' => $topicThermo->id, 'title' => 'Entropy Laws'], ['type' => 'module', 'media_url' => '']);
    }
}
