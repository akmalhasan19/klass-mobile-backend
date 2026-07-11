<?php

namespace Database\Seeders;

use Illuminate\Database\Console\Seeds\WithoutModelEvents;
use Illuminate\Database\Seeder;

class SystemSettingsSeeder extends Seeder
{
    /**
     * Run the database seeds.
     */
    public function run(): void
    {
        $settings = [
            // General
            [
                'key' => 'site_name',
                'value' => 'Klass',
                'type' => 'text',
                'group' => 'general',
                'description' => 'Nama aplikasi yang akan ditampilkan di platform.'
            ],
            [
                'key' => 'site_tagline',
                'value' => 'The complete learning ecosystem',
                'type' => 'text',
                'group' => 'general',
                'description' => 'Slogan singkat aplikasi.'
            ],
            [
                'key' => 'contact_email',
                'value' => 'support@klass.com',
                'type' => 'text',
                'group' => 'general',
                'description' => 'Email kontak dukungan pelanggan.'
            ],

            // Features
            [
                'key' => 'maintenance_mode',
                'value' => '0',
                'type' => 'boolean',
                'group' => 'features',
                'description' => 'Aktifkan untuk membatasi akses publik saat perbaikan.'
            ],
            [
                'key' => 'public_registration',
                'value' => '1',
                'type' => 'boolean',
                'group' => 'features',
                'description' => 'Izinkan pengguna baru untuk mendaftar secara mandiri.'
            ],
            [
                'key' => 'marketplace_active',
                'value' => '1',
                'type' => 'boolean',
                'group' => 'features',
                'description' => 'Aktifkan fitur pencarian tugas marketplace.'
            ],

            // API & Integrations
            [
                'key' => 'tmdb_api_key',
                'value' => '',
                'type' => 'text',
                'group' => 'api',
                'description' => 'API Key untuk integrasi data konten dari TMDB.'
            ],
        ];

        foreach ($settings as $setting) {
            \App\Models\SystemSetting::updateOrCreate(
                ['key' => $setting['key']],
                $setting
            );
        }
    }
}
