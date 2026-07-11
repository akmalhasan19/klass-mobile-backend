<?php

namespace App\Console\Commands;

use App\Models\Topic;
use Illuminate\Console\Attributes\Description;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;

#[Signature('app:backfill-topic-ownership {--only-unresolved=1 : Only scan topics without normalized ownership}')]
#[Description('Backfill normalized topic owner_user_id values from legacy teacher_id identifiers')]
class BackfillTopicOwnership extends Command
{
    public function handle(): int
    {
        $processed = 0;
        $normalized = 0;
        $unresolved = 0;

        $query = Topic::query();

        if ((bool) $this->option('only-unresolved')) {
            $query->where(function ($builder): void {
                $builder->whereNull('owner_user_id')
                    ->orWhere('ownership_status', '!=', Topic::OWNERSHIP_STATUS_NORMALIZED);
            });
        }

        $query->orderBy('created_at')->chunk(100, function ($topics) use (&$processed, &$normalized, &$unresolved): void {
            foreach ($topics as $topic) {
                $processed++;

                $topic->syncOwnershipFromLegacyIdentifier();

                if ($topic->isDirty(['owner_user_id', 'ownership_status'])) {
                    $topic->saveQuietly();
                }

                if ($topic->hasNormalizedOwnership()) {
                    $normalized++;
                } else {
                    $unresolved++;
                }
            }
        });

        $this->info(sprintf(
            'Processed %d topics. Normalized: %d. Legacy unresolved: %d.',
            $processed,
            $normalized,
            $unresolved,
        ));

        return self::SUCCESS;
    }
}