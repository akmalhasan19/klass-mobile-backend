<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Phase 1.3: Create freelancer_matches table for storing AI-computed
 * freelancer matching results per media generation.
 *
 * Each row represents a single freelancer's match score against
 * a specific MediaGeneration, enabling audit trails and retry logic
 * for the auto-suggest hiring flow.
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::create('freelancer_matches', function (Blueprint $table) {
            $table->id();
            $table->uuid('media_generation_id');
            $table->unsignedBigInteger('freelancer_id');
            $table->float('match_score')->default(0);
            $table->float('portfolio_relevance_score')->default(0);
            $table->float('success_rate')->default(0);
            $table->timestamps();

            $table->foreign('media_generation_id')
                  ->references('id')
                  ->on('media_generations')
                  ->cascadeOnDelete();

            $table->foreign('freelancer_id')
                  ->references('id')
                  ->on('users')
                  ->cascadeOnDelete();

            $table->index('media_generation_id');
            $table->index('freelancer_id');
            $table->index('match_score');

            // Prevent duplicate match entries for the same generation-freelancer pair
            $table->unique(['media_generation_id', 'freelancer_id']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('freelancer_matches');
    }
};
