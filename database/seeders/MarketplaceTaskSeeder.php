<?php

namespace Database\Seeders;

use App\Models\Content;
use App\Models\MarketplaceTask;
use App\Models\User;
use Illuminate\Database\Seeder;

class MarketplaceTaskSeeder extends Seeder
{
    public function run(): void
    {
        $freelancers = User::whereIn('name', ['Agus S', 'Ani A', 'Budi O', 'Susi'])->get();
        
        $contents = Content::take(5)->get();

        if ($contents->isEmpty()) {
            return;
        }

        MarketplaceTask::firstOrCreate([
            'content_id' => $contents[0]->id,
            'status' => 'open',
            'creator_id' => 'akmal@example.com',
            'attachment_url' => 'https://example.com/asset1.zip',
        ]);

        MarketplaceTask::firstOrCreate([
            'content_id' => $contents[1]->id,
            'status' => 'taken',
            'creator_id' => $freelancers->first()?->id ?? 'system',
            'attachment_url' => 'https://example.com/asset2.zip',
        ]);

        MarketplaceTask::firstOrCreate([
            'content_id' => $contents[2]->id,
            'status' => 'done',
            'creator_id' => $freelancers->last()?->id ?? 'system',
            'attachment_url' => 'https://example.com/asset3.zip',
        ]);
    }
}
