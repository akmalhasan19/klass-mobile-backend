<?php

namespace App\Console\Commands;

use Illuminate\Console\Attributes\Description;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;
use App\Models\User;
use App\Models\Topic;
use App\Models\Content;
use App\Models\MarketplaceTask;
use App\Models\MediaFile;

#[Signature('app:backfill-media-files')]
#[Description('Backfill media url columns into media_files table')]
class BackfillMediaFiles extends Command
{
    /**
     * Execute the console command.
     */
    public function handle()
    {
        $this->info('Starting media backfill process...');

        // 1. Users Avatars
        User::whereNotNull('avatar_url')->chunk(100, function ($users) {
            foreach ($users as $user) {
                MediaFile::firstOrCreate(['file_path' => $user->avatar_url], [
                    'uploader_id' => $user->id,
                    'file_name' => basename($user->avatar_url) ?: 'avatar',
                    'category' => 'avatar',
                    'disk' => 'supabase',
                ]);
            }
        });
        $this->info('Users avatars backfilled.');

        // 2. Topics Thumbnails
        Topic::whereNotNull('thumbnail_url')->chunk(100, function ($topics) {
            foreach ($topics as $topic) {
                MediaFile::firstOrCreate(['file_path' => $topic->thumbnail_url], [
                    'uploader_id' => $topic->owner_user_id,
                    'file_name' => basename($topic->thumbnail_url) ?: 'thumbnail',
                    'category' => 'thumbnail',
                    'disk' => 'supabase',
                ]);
            }
        });
        $this->info('Topics thumbnails backfilled.');

        // 3. Contents Media
        Content::with('topic')->whereNotNull('media_url')->chunk(100, function ($contents) {
            foreach ($contents as $content) {
                MediaFile::firstOrCreate(['file_path' => $content->media_url], [
                    'uploader_id' => $content->topic ? $content->topic->owner_user_id : null,
                    'file_name' => basename($content->media_url) ?: 'content_media',
                    'category' => 'content_media',
                    'disk' => 'supabase',
                ]);
            }
        });
        $this->info('Contents media backfilled.');

        // 4. Marketplace Task Attachments
        MarketplaceTask::whereNotNull('attachment_url')->chunk(100, function ($tasks) {
            foreach ($tasks as $task) {
                $user = User::where('email', $task->creator_id)->first();
                $uploaderId = $user ? $user->id : null;
                
                MediaFile::firstOrCreate(['file_path' => $task->attachment_url], [
                    'uploader_id' => $uploaderId,
                    'file_name' => basename($task->attachment_url) ?: 'task_attachment',
                    'category' => 'task_attachment',
                    'disk' => 'supabase',
                ]);
            }
        });
        $this->info('Marketplace tasks attachments backfilled.');

        $this->info('Backfill complete!');
    }
}
