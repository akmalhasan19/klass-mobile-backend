<?php

namespace Tests\Feature;

use App\Models\Content;
use App\Models\MarketplaceTask;
use App\Models\Topic;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class Phase6EndToEndVerificationTest extends TestCase
{
    use RefreshDatabase;

    public function test_login_to_home_feed_flow_succeeds(): void
    {
        $user = User::factory()->create([
            'password' => 'password123',
        ]);

        $topic = Topic::create([
            'title' => 'Data Science Basics',
            'teacher_id' => 'teacher-01',
        ]);

        $content = Content::create([
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Intro Module',
            'data' => ['summary' => 'intro'],
            'media_url' => 'https://example.com/materials/intro.pdf',
        ]);

        MarketplaceTask::create([
            'content_id' => $content->id,
            'status' => 'open',
            'creator_id' => 'teacher-01',
            'attachment_url' => 'https://example.com/attachments/task.pdf',
        ]);

        $login = $this->postJson('/api/v1/auth/login', [
            'email' => $user->email,
            'password' => 'password123',
        ]);

        $login
            ->assertOk()
            ->assertJsonPath('success', true)
            ->assertJsonPath('data.user.email', $user->email)
            ->assertJsonStructure(['data' => ['token']]);

        $feed = $this->getJson('/api/v1/marketplace-tasks');

        $feed
            ->assertOk()
            ->assertJsonPath('success', true)
            ->assertJsonPath('meta.total', 1)
            ->assertJsonPath('data.0.status', 'open');
    }

    public function test_profile_update_upload_avatar_flow_succeeds(): void
    {
        Storage::fake('supabase');

        $user = User::factory()->create();
        Sanctum::actingAs($user);

        $response = $this->postJson('/api/v1/user/avatar', [
            'file' => UploadedFile::fake()->image('avatar.png', 200, 200),
        ]);

        $response
            ->assertOk()
            ->assertJsonPath('success', true)
            ->assertJsonStructure(['data' => ['avatar_url', 'user']]);

        $avatarUrl = $response->json('data.avatar_url');

        $this->assertNotNull($avatarUrl);
        $this->assertStringContainsString('/avatars/', $avatarUrl);

        $this->assertNotNull($user->fresh()->avatar_url);
        $this->assertNotEmpty(Storage::disk('supabase')->allFiles('avatars'));
    }

    public function test_bookmark_create_read_update_delete_flow_succeeds(): void
    {
        $admin = User::factory()->admin()->create();
        Sanctum::actingAs($admin);

        $topic = Topic::create([
            'title' => 'Bookmarks Topic',
            'teacher_id' => 'teacher-02',
        ]);

        $content = Content::create([
            'topic_id' => $topic->id,
            'type' => 'brief',
            'title' => 'Bookmarkable Content',
            'data' => ['difficulty' => 'easy'],
            'media_url' => 'https://example.com/materials/bookmark.pdf',
        ]);

        $create = $this->postJson('/api/v1/marketplace-tasks', [
            'content_id' => $content->id,
            'status' => 'open',
            'creator_id' => 'user-bookmark',
        ]);

        $create
            ->assertCreated()
            ->assertJsonPath('success', true)
            ->assertJsonPath('data.status', 'open');

        $taskId = $create->json('data.id');

        $list = $this->getJson('/api/v1/marketplace-tasks?content_id=' . $content->id);

        $list
            ->assertOk()
            ->assertJsonPath('meta.total', 1)
            ->assertJsonPath('data.0.id', $taskId);

        $update = $this->putJson('/api/v1/marketplace-tasks/' . $taskId, [
            'status' => 'done',
        ]);

        $update
            ->assertOk()
            ->assertJsonPath('data.status', 'done');

        $delete = $this->deleteJson('/api/v1/marketplace-tasks/' . $taskId);

        $delete
            ->assertOk()
            ->assertJsonPath('success', true);

        $this->getJson('/api/v1/marketplace-tasks/' . $taskId)
            ->assertNotFound();
    }

    public function test_gallery_load_and_search_filter_flow_succeeds(): void
    {
        $topic = Topic::create([
            'title' => 'Visual Design',
            'teacher_id' => 'teacher-03',
        ]);

        Content::create([
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Poster Composition',
            'data' => ['level' => 'beginner'],
            'media_url' => 'https://example.com/gallery/poster.jpg',
        ]);

        Content::create([
            'topic_id' => $topic->id,
            'type' => 'quiz',
            'title' => 'Color Theory Quiz',
            'data' => ['level' => 'intermediate'],
            'media_url' => null,
        ]);

        Content::create([
            'topic_id' => $topic->id,
            'type' => 'brief',
            'title' => 'Poster Brief Advanced',
            'data' => ['level' => 'advanced'],
            'media_url' => 'https://example.com/gallery/brief.pdf',
        ]);

        $gallery = $this->getJson('/api/v1/gallery');

        $gallery
            ->assertOk()
            ->assertJsonPath('success', true)
            ->assertJsonPath('meta.total', 2);

        $galleryFiltered = $this->getJson('/api/v1/gallery?search=Poster&type=module&topic_id=' . $topic->id);

        $galleryFiltered
            ->assertOk()
            ->assertJsonPath('meta.total', 1)
            ->assertJsonPath('data.0.title', 'Poster Composition');

        $topicSearch = $this->getJson('/api/v1/topics?search=visual&teacher_id=teacher-03');

        $topicSearch
            ->assertOk()
            ->assertJsonPath('meta.total', 1)
            ->assertJsonPath('data.0.title', 'Visual Design')
            ->assertJsonPath('data.0.contents_count', 3)
            ->assertJsonMissingPath('data.0.contents.0.title');

        $topicSearchWithContents = $this->getJson('/api/v1/topics?search=visual&teacher_id=teacher-03&include_contents=1');

        $topicSearchWithContents
            ->assertOk()
            ->assertJsonPath('meta.total', 1)
            ->assertJsonPath('data.0.contents.0.title', 'Poster Composition')
            ->assertJsonMissingPath('data.0.contents_count');
    }
}
