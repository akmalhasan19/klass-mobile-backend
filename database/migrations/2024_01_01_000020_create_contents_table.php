<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('contents', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->uuid('topic_id');
            $table->enum('type', ['module', 'quiz', 'brief']);
            $table->json('data')->nullable();
            $table->timestamps();

            $table->foreign('topic_id')
                  ->references('id')
                  ->on('topics')
                  ->onDelete('cascade');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('contents');
    }
};
