<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('topics', function (Blueprint $table) {
            $table->foreignId('sub_subject_id')->nullable()->after('teacher_id')->constrained('sub_subjects')->nullOnDelete();
            $table->index('sub_subject_id');
        });
    }

    public function down(): void
    {
        Schema::table('topics', function (Blueprint $table) {
            $table->dropIndex(['sub_subject_id']);
            $table->dropConstrainedForeignId('sub_subject_id');
        });
    }
};