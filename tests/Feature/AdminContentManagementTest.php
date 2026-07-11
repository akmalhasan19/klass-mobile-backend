<?php

namespace Tests\Feature;

use App\Models\ActivityLog;
use App\Models\Content;
use App\Models\Topic;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminContentManagementTest extends TestCase
{
    use RefreshDatabase;

    public function test_admin_can_rename_reorder_and_toggle_topic_visibility(): void
    {
        $admin = User::factory()->admin()->create();

        $firstTopic = Topic::create([
            'title' => 'Alpha Topic',
            'teacher_id' => 'teacher-alpha',
            'is_published' => true,
            'order' => 1,
        ]);

        $secondTopic = Topic::create([
            'title' => 'Beta Topic',
            'teacher_id' => 'teacher-beta',
            'is_published' => true,
            'order' => 2,
        ]);

        $this->actingAs($admin)
            ->patch(route('admin.topics.update', $firstTopic), [
                'title' => 'Renamed Topic',
                'is_published' => '1',
            ])
            ->assertRedirect(route('admin.topics.index'));

        $this->actingAs($admin)
            ->patch(route('admin.topics.reorder', $firstTopic), [
                'direction' => 'down',
            ])
            ->assertRedirect();

        $this->actingAs($admin)
            ->patch(route('admin.topics.toggle-publish', $firstTopic))
            ->assertRedirect();

        $firstTopic->refresh();
        $secondTopic->refresh();

        $this->assertSame('Renamed Topic', $firstTopic->title);
        $this->assertFalse((bool) $firstTopic->is_published);
        $this->assertSame(2, $firstTopic->order);
        $this->assertSame(1, $secondTopic->order);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'update_topic',
            'subject_type' => Topic::class,
            'subject_id' => $firstTopic->id,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'reorder_topic',
            'subject_type' => Topic::class,
            'subject_id' => $firstTopic->id,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'toggle_topic_publish',
            'subject_type' => Topic::class,
            'subject_id' => $firstTopic->id,
        ]);
    }

    public function test_admin_can_rename_reorder_and_toggle_content_visibility(): void
    {
        $admin = User::factory()->admin()->create();

        $topic = Topic::create([
            'title' => 'Content Topic',
            'teacher_id' => 'teacher-content',
            'order' => 1,
        ]);

        $firstContent = Content::create([
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Alpha Content',
            'data' => ['level' => 'beginner'],
            'media_url' => 'https://example.com/alpha.pdf',
            'is_published' => true,
            'order' => 1,
        ]);

        $secondContent = Content::create([
            'topic_id' => $topic->id,
            'type' => 'brief',
            'title' => 'Beta Content',
            'data' => ['level' => 'advanced'],
            'media_url' => 'https://example.com/beta.pdf',
            'is_published' => true,
            'order' => 2,
        ]);

        $this->actingAs($admin)
            ->patch(route('admin.contents.update', $firstContent), [
                'title' => 'Renamed Content',
                'topic_id' => $topic->id,
                'is_published' => '1',
            ])
            ->assertRedirect(route('admin.contents.index'));

        $this->actingAs($admin)
            ->patch(route('admin.contents.reorder', $firstContent), [
                'direction' => 'down',
            ])
            ->assertRedirect();

        $this->actingAs($admin)
            ->patch(route('admin.contents.toggle-publish', $firstContent))
            ->assertRedirect();

        $firstContent->refresh();
        $secondContent->refresh();

        $this->assertSame('Renamed Content', $firstContent->title);
        $this->assertFalse((bool) $firstContent->is_published);
        $this->assertSame(2, $firstContent->order);
        $this->assertSame(1, $secondContent->order);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'update_content',
            'subject_type' => Content::class,
            'subject_id' => $firstContent->id,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'reorder_content',
            'subject_type' => Content::class,
            'subject_id' => $firstContent->id,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'toggle_content_publish',
            'subject_type' => Content::class,
            'subject_id' => $firstContent->id,
        ]);

        $log = ActivityLog::where('action', 'reorder_content')->first();

        $this->assertSame('down', $log->metadata['direction']);
    }
}