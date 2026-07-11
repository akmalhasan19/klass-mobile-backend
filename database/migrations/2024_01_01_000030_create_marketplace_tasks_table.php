<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('marketplace_tasks', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->uuid('content_id');
            $table->enum('status', ['open', 'taken', 'done'])->default('open');
            $table->string('creator_id')->nullable();
            $table->timestamps();

            $table->foreign('content_id')
                  ->references('id')
                  ->on('contents')
                  ->onDelete('cascade');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('marketplace_tasks');
    }
};
