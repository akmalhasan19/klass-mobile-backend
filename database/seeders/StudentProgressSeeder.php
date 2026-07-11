<?php

namespace Database\Seeders;

use App\Models\StudentProgress;
use Illuminate\Database\Seeder;
use Illuminate\Support\Carbon;

class StudentProgressSeeder extends Seeder
{
    public function run(): void
    {
        $students = [
            ['student_name' => 'Michael D', 'score' => 95],
            ['student_name' => 'John Doe', 'score' => 88],
            ['student_name' => 'Jane Smith', 'score' => 75],
            ['student_name' => 'Alex Wong', 'score' => 100],
            ['student_name' => 'Emily Chen', 'score' => 92],
        ];

        foreach ($students as $index => $data) {
            StudentProgress::firstOrCreate([
                'student_name' => $data['student_name']
            ], [
                'score' => $data['score'],
                'completion_date' => Carbon::now()->subDays($index * 2),
            ]);
        }
    }
}
