<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Phase 1.1: Add parent-child generation tracking to media_generations.
 *
 * New columns:
 * - generated_from_id: nullable FK pointing to the parent MediaGeneration (self-referencing)
 * - is_regeneration: boolean flag to quickly identify regenerated records
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::table('media_generations', function (Blueprint $table) {
            $table->uuid('generated_from_id')->nullable()->after('id');
            $table->boolean('is_regeneration')->default(false)->after('generated_from_id');

            $table->foreign('generated_from_id')
                  ->references('id')
                  ->on('media_generations')
                  ->nullOnDelete();

            $table->index('generated_from_id');
        });
    }

    public function down(): void
    {
        Schema::table('media_generations', function (Blueprint $table) {
            $table->dropForeign(['generated_from_id']);
            $table->dropIndex(['generated_from_id']);
            $table->dropColumn(['generated_from_id', 'is_regeneration']);
        });
    }
};
