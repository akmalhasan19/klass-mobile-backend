<?php

namespace Tests\Feature;

use App\Models\ActivityLog;
use App\Models\Content;
use App\Models\MarketplaceTask;
use App\Models\Topic;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminTaskModerationTest extends TestCase
{
    use RefreshDatabase;

    public function test_admin_can_filter_review_and_moderate_marketplace_tasks(): void
    {
        $admin = User::factory()->admin()->create();

        $topic = Topic::create([
            'title' => 'Task Topic',
            'teacher_id' => 'teacher-task',
            'order' => 1,
        ]);

        $targetContent = Content::create([
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Illustration Task',
            'data' => ['mode' => 'target'],
            'media_url' => 'https://example.com/illustration.pdf',
            'order' => 1,
        ]);

        $otherContent = Content::create([
            'topic_id' => $topic->id,
            'type' => 'brief',
            'title' => 'Copywriting Task',
            'data' => ['mode' => 'other'],
            'media_url' => 'https://example.com/copywriting.pdf',
            'order' => 2,
        ]);

        $targetTask = MarketplaceTask::create([
            'content_id' => $targetContent->id,
            'status' => 'open',
            'creator_id' => 'creator-1',
            'attachment_url' => 'https://example.com/open.pdf',
        ]);

        MarketplaceTask::create([
            'content_id' => $otherContent->id,
            'status' => 'done',
            'creator_id' => 'creator-2',
            'attachment_url' => 'https://example.com/done.pdf',
        ]);

        $this->actingAs($admin)
            ->get(route('admin.tasks.index', ['status' => 'open', 'search' => 'Illustration']))
            ->assertOk()
            ->assertSeeText('Illustration Task')
            ->assertDontSeeText('Copywriting Task');

        $this->actingAs($admin)
            ->get(route('admin.tasks.show', $targetTask))
            ->assertOk()
            ->assertSeeText('Illustration Task')
            ->assertSeeText('Override Status');

        $this->actingAs($admin)
            ->patch(route('admin.tasks.update-status', $targetTask), [
                'status' => 'taken',
            ])
            ->assertRedirect();

        $this->assertDatabaseHas('marketplace_tasks', [
            'id' => $targetTask->id,
            'status' => 'taken',
        ]);

        $this->actingAs($admin)
            ->delete(route('admin.tasks.destroy', $targetTask))
            ->assertRedirect(route('admin.tasks.index'));

        $this->assertDatabaseMissing('marketplace_tasks', [
            'id' => $targetTask->id,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'update_task_status',
            'subject_type' => MarketplaceTask::class,
            'subject_id' => $targetTask->id,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'delete_task',
            'subject_type' => MarketplaceTask::class,
            'subject_id' => $targetTask->id,
        ]);

        $statusLog = ActivityLog::where('action', 'update_task_status')->first();

        $this->assertSame('open', $statusLog->metadata['old_status']);
        $this->assertSame('taken', $statusLog->metadata['new_status']);
    }
}