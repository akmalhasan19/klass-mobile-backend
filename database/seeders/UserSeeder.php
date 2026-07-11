<?php

namespace Database\Seeders;

use App\Models\Subject;
use App\Models\User;
use App\Services\FileUploadService;
use Illuminate\Database\Seeder;
use Illuminate\Support\Facades\Hash;

/**
 * UserSeeder
 *
 * Membuat user awal yang merepresentasikan:
 * - 1 akun admin (Klass Admin)
 * - 1 akun demo teacher (Dr. Sarah Jenkins — profil utama app)
 * - 1 akun demo freelancer (Rina Freelancer — demo freelancer login)
 * - 4 freelancer/tutor (Agus, Ani, Budi, Susi — dari home screen)
 * - 2 teacher (Elena Rodriguez, Marcus Chen — dari search screen)
 */
class UserSeeder extends Seeder
{
    public function run(): void
    {
        $uploadService = app(FileUploadService::class);
        $subjects = Subject::query()
            ->whereIn('slug', ['science', 'history', 'mathematics'])
            ->get()
            ->keyBy('slug');

        $users = [
            [
                'name' => 'Klass Admin',
                'email' => 'admin@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => null,
                'primary_subject_id' => null,
                'role' => User::ROLE_ADMIN,
                'security_question' => 'Siapa nama hewan peliharaan pertama Anda?',
                'security_answer' => Hash::make('kucing'),
            ],

            // ── Demo Teacher ─────────────────────────────────────────
            // Profil utama yang ditampilkan di profile_screen.dart
            [
                'name' => 'Dr. Sarah Jenkins',
                'email' => 'sarah.jenkins@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => $uploadService->generatePublicUrl('avatars/ani.png'),
                'primary_subject_id' => $subjects->get('science')?->id,
                'role' => User::ROLE_TEACHER,
                'security_question' => 'Apa nama sekolah pertama Anda?',
                'security_answer' => Hash::make('sdn1'),
            ],

            // ── Demo Freelancer ──────────────────────────────────────
            // Akun demo untuk login sebagai freelancer
            [
                'name' => 'Rina Kreatif',
                'email' => 'rina@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => $uploadService->generatePublicUrl('avatars/susi.png'),
                'primary_subject_id' => null,
                'role' => User::ROLE_FREELANCER,
                'security_question' => 'Apa warna favorit Anda?',
                'security_answer' => Hash::make('biru'),
            ],

            // ── Freelancers (ditampilkan di home_screen.dart) ────────
            [
                'name' => 'Agus S',
                'email' => 'agus@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => $uploadService->generatePublicUrl('avatars/agus.png'),
                'primary_subject_id' => null,
                'role' => User::ROLE_FREELANCER,
            ],
            [
                'name' => 'Ani A',
                'email' => 'ani@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => $uploadService->generatePublicUrl('avatars/ani.png'),
                'primary_subject_id' => null,
                'role' => User::ROLE_FREELANCER,
            ],
            [
                'name' => 'Budi O',
                'email' => 'budi@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => $uploadService->generatePublicUrl('avatars/budi.png'),
                'primary_subject_id' => null,
                'role' => User::ROLE_FREELANCER,
            ],
            [
                'name' => 'Susi',
                'email' => 'susi@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => $uploadService->generatePublicUrl('avatars/susi.png'),
                'primary_subject_id' => null,
                'role' => User::ROLE_FREELANCER,
            ],

            // ── Teachers (ditampilkan di search_screen.dart) ────────
            [
                'name' => 'Elena Rodriguez',
                'email' => 'elena@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => null,
                'primary_subject_id' => $subjects->get('history')?->id,
                'role' => User::ROLE_TEACHER,
            ],
            [
                'name' => 'Marcus Chen',
                'email' => 'marcus@klass.id',
                'password' => Hash::make('password'),
                'avatar_url' => null,
                'primary_subject_id' => $subjects->get('mathematics')?->id,
                'role' => User::ROLE_TEACHER,
            ],
        ];

        foreach ($users as $userData) {
            User::updateOrCreate(
                ['email' => $userData['email']],
                $userData,
            );
        }
    }
}

