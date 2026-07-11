<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('media_generations', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignId('teacher_id')->constrained('users')->cascadeOnDelete();
            $table->foreignId('subject_id')->nullable()->constrained('subjects')->nullOnDelete();
            $table->foreignId('sub_subject_id')->nullable()->constrained('sub_subjects')->nullOnDelete();
            $table->foreignUuid('topic_id')->nullable()->constrained('topics')->nullOnDelete();
            $table->foreignUuid('content_id')->nullable()->constrained('contents')->nullOnDelete();
            $table->foreignId('recommended_project_id')->nullable()->constrained('recommended_projects')->nullOnDelete();
            $table->text('raw_prompt');
            $table->string('request_fingerprint', 64);
            $table->string('active_duplicate_key', 64)->nullable()->unique();
            $table->string('preferred_output_type')->default('auto');
            $table->string('resolved_output_type')->nullable();
            $table->string('status')->default('queued');
            $table->string('llm_provider')->nullable();
            $table->string('llm_model')->nullable();
            $table->string('generator_provider')->nullable();
            $table->string('generator_model')->nullable();
            $table->json('interpretation_payload')->nullable();
            $table->json('generation_spec_payload')->nullable();
            $table->json('delivery_payload')->nullable();
            $table->json('generator_service_response')->nullable();
            $table->string('storage_path')->nullable();
            $table->text('file_url')->nullable();
            $table->text('thumbnail_url')->nullable();
            $table->string('mime_type')->nullable();
            $table->string('error_code')->nullable();
            $table->text('error_message')->nullable();
            $table->timestamps();

            $table->index(['teacher_id', 'created_at']);
            $table->index(['status', 'created_at']);
            $table->index(['teacher_id', 'request_fingerprint']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('media_generations');
    }
};