<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('users', function (Blueprint $table) {
            $table->foreignId('primary_subject_id')->nullable()->after('avatar_url')->constrained('subjects')->nullOnDelete();
            $table->index('primary_subject_id');
        });
    }

    public function down(): void
    {
        Schema::table('users', function (Blueprint $table) {
            $table->dropIndex(['primary_subject_id']);
            $table->dropConstrainedForeignId('primary_subject_id');
        });
    }
};