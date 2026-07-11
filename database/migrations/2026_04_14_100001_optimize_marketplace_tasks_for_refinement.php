<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Phase 1.2: Enhance marketplace_tasks for freelancer refinement workflow.
 *
 * New columns:
 * - task_type: enum distinguishing 'bid' (open posting) vs 'suggestion' (auto-matched)
 * - description: text field for refinement requirements from the teacher
 * - suggested_freelancer_id: nullable FK for auto-suggested freelancer assignment
 * - media_generation_id: FK linking the task to its source MediaGeneration
 *
 * Also updates the status enum to include the new statuses needed for the refinement flow.
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::table('marketplace_tasks', function (Blueprint $table) {
            $table->string('task_type', 20)->default('bid')->after('status');
            $table->text('description')->nullable()->after('task_type');
            $table->unsignedBigInteger('suggested_freelancer_id')->nullable()->after('creator_id');
            $table->uuid('media_generation_id')->nullable()->after('content_id');

            $table->foreign('suggested_freelancer_id')
                  ->references('id')
                  ->on('users')
                  ->nullOnDelete();

            $table->foreign('media_generation_id')
                  ->references('id')
                  ->on('media_generations')
                  ->nullOnDelete();

            $table->index('task_type');
            $table->index('media_generation_id');
            $table->index('suggested_freelancer_id');
        });
    }

    public function down(): void
    {
        Schema::table('marketplace_tasks', function (Blueprint $table) {
            $table->dropForeign(['suggested_freelancer_id']);
            $table->dropForeign(['media_generation_id']);
            $table->dropIndex(['task_type']);
            $table->dropIndex(['media_generation_id']);
            $table->dropIndex(['suggested_freelancer_id']);
            $table->dropColumn(['task_type', 'description', 'suggested_freelancer_id', 'media_generation_id']);
        });
    }
};
