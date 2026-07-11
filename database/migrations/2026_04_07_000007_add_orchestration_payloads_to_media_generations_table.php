<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('media_generations', function (Blueprint $table) {
            $table->json('interpretation_audit_payload')->nullable();
            $table->json('decision_payload')->nullable();
        });
    }

    public function down(): void
    {
        Schema::table('media_generations', function (Blueprint $table) {
            $table->dropColumn(['interpretation_audit_payload', 'decision_payload']);
        });
    }
};