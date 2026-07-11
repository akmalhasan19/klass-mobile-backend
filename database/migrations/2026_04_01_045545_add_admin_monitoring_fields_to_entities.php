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
        Schema::table('topics', function (Blueprint $table) {
            $table->boolean('is_published')->default(true)->after('thumbnail_url');
            $table->integer('order')->default(0)->after('is_published');
            $table->index('order');
        });

        Schema::table('contents', function (Blueprint $table) {
            $table->boolean('is_published')->default(true)->after('media_url');
            $table->integer('order')->default(0)->after('is_published');
            $table->index('order');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::table('topics', function (Blueprint $table) {
            $table->dropIndex(['order']);
            $table->dropColumn(['is_published', 'order']);
        });

        Schema::table('contents', function (Blueprint $table) {
            $table->dropIndex(['order']);
            $table->dropColumn(['is_published', 'order']);
        });
    }
};
