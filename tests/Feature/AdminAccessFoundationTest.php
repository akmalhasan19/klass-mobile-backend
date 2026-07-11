<?php

namespace Tests\Feature;

use App\Models\Content;
use App\Models\Topic;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\UploadedFile;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class AdminAccessFoundationTest extends TestCase
{
    use RefreshDatabase;

    public function test_register_assigns_default_user_role(): void
    {
        $response = $this->postJson('/api/v1/auth/register', [
            'name' => 'Phase One User',
            'email' => 'phase1@example.com',
            'password' => 'password123',
            'password_confirmation' => 'password123',
        ]);

        $response
            ->assertCreated()
            ->assertJsonPath('data.user.role', User::ROLE_TEACHER)
            ->assertJsonPath('data.user.is_admin', false);

        $this->assertDatabaseHas('users', [
            'email' => 'phase1@example.com',
            'role' => User::ROLE_TEACHER,
        ]);
    }

    public function test_admin_web_area_is_restricted_to_admin_users(): void
    {
        // Tanpa autentikasi → redirect ke halaman login admin
        $this->get('/admin')->assertRedirect(route('admin.login'));

        // User biasa (login tapi bukan admin) → 403
        $user = User::factory()->create();
        $this->actingAs($user)->get('/admin')->assertForbidden();

        // Admin → melihat dashboard
        $admin = User::factory()->admin()->create();
        $this->actingAs($admin)->get('/admin')->assertOk();
    }

    public function test_write_routes_are_not_public_and_admin_boundaries_are_enforced(): void
    {
        $topic = Topic::create([
            'title' => 'Security Topic',
            'teacher_id' => 'teacher-01',
        ]);

        $content = Content::create([
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Security Content',
            'data' => ['summary' => 'secure'],
            'media_url' => 'https://example.com/materials/secure.pdf',
        ]);

        $this->postJson('/api/v1/topics', [
            'title' => 'Guest Topic',
            'teacher_id' => 'teacher-guest',
        ])->assertUnauthorized();

        $this->postJson('/api/v1/marketplace-tasks', [
            'content_id' => $content->id,
            'status' => 'open',
            'creator_id' => 'guest',
        ])->assertUnauthorized();

        $this->postJson('/api/v1/upload/gallery', [
            'file' => UploadedFile::fake()->image('gallery.png'),
        ])->assertUnauthorized();

        $user = User::factory()->create();
        Sanctum::actingAs($user);

        $this->postJson('/api/v1/topics', [
            'title' => 'User Topic',
            'teacher_id' => 'teacher-user',
        ])->assertCreated();

        $this->putJson('/api/v1/topics/' . $topic->id, [
            'title' => 'Updated by User',
        ])->assertForbidden();

        $this->postJson('/api/v1/contents', [
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Blocked Content',
            'data' => ['summary' => 'blocked'],
        ])->assertForbidden();
    }

    public function test_admin_can_execute_admin_only_api_actions(): void
    {
        $topic = Topic::create([
            'title' => 'Admin Topic',
            'teacher_id' => 'teacher-admin',
        ]);

        $admin = User::factory()->admin()->create();
        Sanctum::actingAs($admin);

        $updateTopic = $this->putJson('/api/v1/topics/' . $topic->id, [
            'title' => 'Admin Updated Topic',
        ]);

        $updateTopic
            ->assertOk()
            ->assertJsonPath('data.title', 'Admin Updated Topic');

        $createContent = $this->postJson('/api/v1/contents', [
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Admin Content',
            'data' => ['summary' => 'allowed'],
        ]);

        $createContent
            ->assertCreated()
            ->assertJsonPath('data.title', 'Admin Content');
    }
}