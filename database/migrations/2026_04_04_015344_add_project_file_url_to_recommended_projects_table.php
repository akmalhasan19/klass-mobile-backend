<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::table('recommended_projects', function (Blueprint $table) {
            $table->string('project_file_url')->nullable()->after('thumbnail_url');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('recommended_projects', function (Blueprint $table) {
            $table->dropColumn('project_file_url');
        });
    }
};
