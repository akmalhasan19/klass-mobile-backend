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
        Schema::create('system_recommendation_assignments', function (Blueprint $table) {
            $table->id();
            $table->foreignId('user_id')->constrained('users')->cascadeOnDelete();
            $table->string('recommendation_key');
            $table->string('recommendation_item_id');
            $table->string('source_type');
            $table->string('source_reference');
            $table->foreignId('subject_id')->nullable()->constrained('subjects')->nullOnDelete();
            $table->foreignId('sub_subject_id')->nullable()->constrained('sub_subjects')->nullOnDelete();
            $table->timestamp('first_distributed_at');
            $table->timestamp('last_distributed_at');
            $table->timestamps();

            $table->unique(['user_id', 'recommendation_key']);
            $table->index(['source_type', 'source_reference']);
            $table->index(['sub_subject_id', 'recommendation_key']);
            $table->index('subject_id');
            $table->index('last_distributed_at');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('system_recommendation_assignments');
    }
};