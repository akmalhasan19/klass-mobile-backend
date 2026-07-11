<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('topics', function (Blueprint $table) {
            $table->text('thumbnail_url')->nullable()->change();
        });

        Schema::table('contents', function (Blueprint $table) {
            $table->text('media_url')->nullable()->change();
        });

        Schema::table('recommended_projects', function (Blueprint $table) {
            $table->text('thumbnail_url')->nullable()->change();
            $table->text('project_file_url')->nullable()->change();
        });
    }

    public function down(): void
    {
        Schema::table('topics', function (Blueprint $table) {
            $table->string('thumbnail_url')->nullable()->change();
        });

        Schema::table('contents', function (Blueprint $table) {
            $table->string('media_url')->nullable()->change();
        });

        Schema::table('recommended_projects', function (Blueprint $table) {
            $table->string('thumbnail_url')->nullable()->change();
            $table->string('project_file_url')->nullable()->change();
        });
    }
};