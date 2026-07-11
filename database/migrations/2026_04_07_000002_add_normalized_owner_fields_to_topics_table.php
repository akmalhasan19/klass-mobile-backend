<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('topics', function (Blueprint $table) {
            $table->foreignId('owner_user_id')->nullable()->constrained('users')->nullOnDelete();
            $table->string('ownership_status')->default('legacy_unresolved');
            $table->index('ownership_status');
        });

        DB::table('topics')
            ->select(['id', 'teacher_id'])
            ->orderBy('created_at')
            ->get()
            ->each(function (object $topic): void {
                $legacyTeacherId = trim((string) $topic->teacher_id);
                $ownerUserId = null;

                if ($legacyTeacherId !== '' && preg_match('/^\d+$/', $legacyTeacherId) === 1) {
                    $ownerUserId = DB::table('users')
                        ->where('id', (int) $legacyTeacherId)
                        ->value('id');
                }

                if ($ownerUserId === null && $legacyTeacherId !== '' && filter_var($legacyTeacherId, FILTER_VALIDATE_EMAIL)) {
                    $ownerUserId = DB::table('users')
                        ->whereRaw('LOWER(email) = ?', [strtolower($legacyTeacherId)])
                        ->value('id');
                }

                DB::table('topics')
                    ->where('id', $topic->id)
                    ->update([
                        'owner_user_id' => $ownerUserId,
                        'ownership_status' => $ownerUserId !== null ? 'normalized' : 'legacy_unresolved',
                    ]);
            });
    }

    public function down(): void
    {
        Schema::table('topics', function (Blueprint $table) {
            $table->dropIndex(['ownership_status']);
            $table->dropConstrainedForeignId('owner_user_id');
            $table->dropColumn('ownership_status');
        });
    }
};