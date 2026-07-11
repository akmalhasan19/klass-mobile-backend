<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('media_generations', function (Blueprint $table) {
            $table->json('orchestration_audit_payload')->nullable()->after('decision_payload');
        });
    }

    public function down(): void
    {
        Schema::table('media_generations', function (Blueprint $table) {
            $table->dropColumn('orchestration_audit_payload');
        });
    }
};