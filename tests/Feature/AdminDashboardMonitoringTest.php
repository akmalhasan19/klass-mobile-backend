<?php

namespace Tests\Feature;

use App\Models\ActivityLog;
use App\Models\Content;
use App\Models\MarketplaceTask;
use App\Models\MediaFile;
use App\Models\Topic;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminDashboardMonitoringTest extends TestCase
{
    use RefreshDatabase;

    public function test_dashboard_displays_summary_counts_and_recent_operational_data(): void
    {
        $admin = User::factory()->admin()->create([
            'name' => 'Admin Operator',
        ]);

        $recentUser = User::factory()->create([
            'name' => 'Recent User',
            'email' => 'recent@klass.id',
        ]);

        $topic = Topic::create([
            'title' => 'Recent Topic',
            'teacher_id' => 'teacher-dashboard',
            'order' => 1,
        ]);

        $content = Content::create([
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Dashboard Content',
            'data' => ['summary' => 'dashboard'],
            'media_url' => 'https://example.com/dashboard.pdf',
            'order' => 1,
        ]);

        $task = MarketplaceTask::create([
            'content_id' => $content->id,
            'status' => 'open',
            'creator_id' => 'creator-dashboard',
            'attachment_url' => 'https://example.com/task.pdf',
        ]);

        $media = MediaFile::create([
            'uploader_id' => $admin->id,
            'file_path' => 'materials/dashboard.pdf',
            'file_name' => 'dashboard.pdf',
            'mime_type' => 'application/pdf',
            'size' => 2048,
            'disk' => 'public',
            'category' => 'materials',
        ]);

        ActivityLog::create([
            'actor_id' => $admin->id,
            'action' => 'update_topic',
            'subject_type' => Topic::class,
            'subject_id' => $topic->id,
            'metadata' => ['title' => $topic->title],
        ]);

        $response = $this->actingAs($admin)->get(route('admin.dashboard'));

        $response
            ->assertOk()
            ->assertViewHas('usersCount', User::count())
            ->assertViewHas('topicsCount', Topic::count())
            ->assertViewHas('contentsCount', Content::count())
            ->assertViewHas('tasksCount', MarketplaceTask::count())
            ->assertViewHas('mediaCount', MediaFile::count())
            ->assertViewHas('activityCount', ActivityLog::count())
            ->assertSeeText('Recent User')
            ->assertSeeText('Dashboard Content');

        $this->assertTrue($response->viewData('recentUsers')->contains('email', $recentUser->email));
        $this->assertTrue($response->viewData('recentContents')->contains('title', $content->title));
        $this->assertTrue($response->viewData('recentTasks')->contains('id', $task->id));
        $this->assertTrue($response->viewData('recentMedia')->contains('file_name', $media->file_name));
        $this->assertTrue($response->viewData('recentActivity')->contains('action', 'update_topic'));
    }

    public function test_dashboard_period_filter_limits_summary_counts_to_recent_records(): void
    {
        $admin = User::factory()->admin()->create();
        $admin->forceFill([
            'created_at' => now()->subDays(40),
            'updated_at' => now()->subDays(40),
        ])->saveQuietly();

        $recentUser = User::factory()->create([
            'name' => 'Fresh User',
            'email' => 'fresh@klass.id',
        ]);

        $oldUser = User::factory()->create([
            'name' => 'Old User',
            'email' => 'old@klass.id',
        ]);
        $oldUser->forceFill([
            'created_at' => now()->subDays(12),
            'updated_at' => now()->subDays(12),
        ])->saveQuietly();

        $recentTopic = Topic::create([
            'title' => 'Fresh Topic',
            'teacher_id' => 'teacher-recent',
            'order' => 1,
        ]);

        $oldTopic = Topic::create([
            'title' => 'Old Topic',
            'teacher_id' => 'teacher-old',
            'order' => 2,
        ]);
        $oldTopic->forceFill([
            'created_at' => now()->subDays(12),
            'updated_at' => now()->subDays(12),
        ])->saveQuietly();

        $recentContent = Content::create([
            'topic_id' => $recentTopic->id,
            'type' => 'module',
            'title' => 'Fresh Content',
            'data' => ['summary' => 'recent'],
            'media_url' => 'https://example.com/fresh.pdf',
            'order' => 1,
        ]);

        $oldContent = Content::create([
            'topic_id' => $recentTopic->id,
            'type' => 'brief',
            'title' => 'Old Content',
            'data' => ['summary' => 'old'],
            'media_url' => 'https://example.com/old.pdf',
            'order' => 2,
        ]);
        $oldContent->forceFill([
            'created_at' => now()->subDays(12),
            'updated_at' => now()->subDays(12),
        ])->saveQuietly();

        MarketplaceTask::create([
            'content_id' => $recentContent->id,
            'status' => 'open',
            'creator_id' => 'recent-task',
        ]);

        $oldTask = MarketplaceTask::create([
            'content_id' => $recentContent->id,
            'status' => 'done',
            'creator_id' => 'old-task',
        ]);
        $oldTask->forceFill([
            'created_at' => now()->subDays(12),
            'updated_at' => now()->subDays(12),
        ])->saveQuietly();

        MediaFile::create([
            'uploader_id' => $admin->id,
            'file_path' => 'materials/fresh.pdf',
            'file_name' => 'fresh.pdf',
            'mime_type' => 'application/pdf',
            'size' => 1024,
            'disk' => 'public',
            'category' => 'materials',
        ]);

        $oldMedia = MediaFile::create([
            'uploader_id' => $admin->id,
            'file_path' => 'materials/old.pdf',
            'file_name' => 'old.pdf',
            'mime_type' => 'application/pdf',
            'size' => 1024,
            'disk' => 'public',
            'category' => 'materials',
        ]);
        $oldMedia->forceFill([
            'created_at' => now()->subDays(12),
            'updated_at' => now()->subDays(12),
        ])->saveQuietly();

        ActivityLog::create([
            'actor_id' => $admin->id,
            'action' => 'recent_event',
            'subject_type' => Topic::class,
            'subject_id' => $recentTopic->id,
            'metadata' => ['scope' => 'recent'],
        ]);

        $oldActivity = ActivityLog::create([
            'actor_id' => $admin->id,
            'action' => 'old_event',
            'subject_type' => Topic::class,
            'subject_id' => $recentTopic->id,
            'metadata' => ['scope' => 'old'],
        ]);
        $oldActivity->forceFill([
            'created_at' => now()->subDays(12),
            'updated_at' => now()->subDays(12),
        ])->saveQuietly();

        $response = $this->actingAs($admin)->get(route('admin.dashboard', ['period' => '7_days']));

        $response
            ->assertOk()
            ->assertViewHas('period', '7_days')
            ->assertViewHas('usersCount', 1)
            ->assertViewHas('topicsCount', 1)
            ->assertViewHas('contentsCount', 1)
            ->assertViewHas('tasksCount', 1)
            ->assertViewHas('mediaCount', 1)
            ->assertViewHas('activityCount', 1)
            ->assertSeeText('Fresh User')
            ->assertDontSeeText('Old User')
            ->assertSeeText('Fresh Content')
            ->assertDontSeeText('Old Content');

        $this->assertTrue($response->viewData('recentUsers')->contains('email', $recentUser->email));
        $this->assertFalse($response->viewData('recentUsers')->contains('email', $oldUser->email));
        $this->assertTrue($response->viewData('recentContents')->contains('title', $recentContent->title));
        $this->assertFalse($response->viewData('recentContents')->contains('title', $oldContent->title));
        $this->assertTrue($response->viewData('recentTasks')->contains('status', 'open'));
        $this->assertFalse($response->viewData('recentTasks')->contains('status', 'done'));
        $this->assertTrue($response->viewData('recentMedia')->contains('file_name', 'fresh.pdf'));
        $this->assertFalse($response->viewData('recentMedia')->contains('file_name', 'old.pdf'));
        $this->assertTrue($response->viewData('recentActivity')->contains('action', 'recent_event'));
        $this->assertFalse($response->viewData('recentActivity')->contains('action', 'old_event'));
    }
}