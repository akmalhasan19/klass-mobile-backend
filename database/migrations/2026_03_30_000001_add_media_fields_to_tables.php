<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Menambahkan kolom media/URL ke tabel-tabel utama.
 *
 * Kolom baru per tabel:
 * - users: avatar_url (foto profil dari Supabase Storage)
 * - topics: thumbnail_url (gambar thumbnail topik)
 * - contents: title (judul content), media_url (link ke file/media di bucket)
 * - marketplace_tasks: attachment_url (lampiran tugas)
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::table('users', function (Blueprint $table) {
            $table->string('avatar_url')->nullable()->after('password');
        });

        Schema::table('topics', function (Blueprint $table) {
            $table->string('thumbnail_url')->nullable()->after('teacher_id');
        });

        Schema::table('contents', function (Blueprint $table) {
            $table->string('title')->nullable()->after('type');
            $table->string('media_url')->nullable()->after('data');
        });

        Schema::table('marketplace_tasks', function (Blueprint $table) {
            $table->string('attachment_url')->nullable()->after('creator_id');
        });
    }

    public function down(): void
    {
        Schema::table('users', function (Blueprint $table) {
            $table->dropColumn('avatar_url');
        });

        Schema::table('topics', function (Blueprint $table) {
            $table->dropColumn('thumbnail_url');
        });

        Schema::table('contents', function (Blueprint $table) {
            $table->dropColumn(['title', 'media_url']);
        });

        Schema::table('marketplace_tasks', function (Blueprint $table) {
            $table->dropColumn('attachment_url');
        });
    }
};
