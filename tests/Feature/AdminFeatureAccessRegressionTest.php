<?php

namespace Tests\Feature;

use App\Models\Content;
use App\Models\HomepageSection;
use App\Models\MarketplaceTask;
use App\Models\MediaFile;
use App\Models\RecommendedProject;
use App\Models\Topic;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminFeatureAccessRegressionTest extends TestCase
{
    use RefreshDatabase;

    public function test_non_admin_user_cannot_access_admin_management_routes_or_actions(): void
    {
        $user = User::factory()->create();

        $managedUser = User::factory()->create();
        $topic = Topic::create([
            'title' => 'Restricted Topic',
            'teacher_id' => 'teacher-restricted',
            'order' => 1,
        ]);
        $content = Content::create([
            'topic_id' => $topic->id,
            'type' => 'module',
            'title' => 'Restricted Content',
            'data' => ['scope' => 'restricted'],
            'media_url' => 'https://example.com/restricted.pdf',
            'order' => 1,
        ]);
        $task = MarketplaceTask::create([
            'content_id' => $content->id,
            'status' => 'open',
            'creator_id' => 'restricted-user',
        ]);
        $media = MediaFile::create([
            'uploader_id' => $managedUser->id,
            'file_path' => 'materials/restricted.pdf',
            'file_name' => 'restricted.pdf',
            'mime_type' => 'application/pdf',
            'size' => 512,
            'disk' => 'public',
            'category' => 'materials',
        ]);
        $section = HomepageSection::create([
            'key' => 'restricted_section',
            'label' => 'Restricted Section',
            'position' => 1,
            'is_enabled' => true,
            'data_source' => 'restricted',
        ]);
        $recommendedProject = RecommendedProject::factory()->create([
            'title' => 'Locked Recommendation',
            'is_active' => true,
        ]);

        $this->actingAs($user)->get(route('admin.users.index'))->assertForbidden();
        $this->actingAs($user)->get(route('admin.users.show', $managedUser))->assertForbidden();
        $this->actingAs($user)->get(route('admin.topics.index'))->assertForbidden();
        $this->actingAs($user)->get(route('admin.contents.index'))->assertForbidden();
        $this->actingAs($user)->get(route('admin.tasks.index'))->assertForbidden();
        $this->actingAs($user)->get(route('admin.tasks.show', $task))->assertForbidden();
        $this->actingAs($user)->get(route('admin.media.index'))->assertForbidden();
        $this->actingAs($user)->get(route('admin.media-generations.index'))->assertForbidden();
        $this->actingAs($user)->get(route('admin.activity-logs.index'))->assertForbidden();
        $this->actingAs($user)->get(route('admin.homepage-sections.index'))->assertForbidden();

        $this->actingAs($user)
            ->patch(route('admin.users.update-role', $managedUser), ['role' => User::ROLE_ADMIN])
            ->assertForbidden();

        $this->actingAs($user)
            ->patch(route('admin.topics.update', $topic), ['title' => 'Should Fail'])
            ->assertForbidden();

        $this->actingAs($user)
            ->patch(route('admin.contents.update', $content), ['title' => 'Should Fail', 'topic_id' => $topic->id])
            ->assertForbidden();

        $this->actingAs($user)
            ->patch(route('admin.tasks.update-status', $task), ['status' => 'taken'])
            ->assertForbidden();

        $this->actingAs($user)
            ->delete(route('admin.tasks.destroy', $task))
            ->assertForbidden();

        $this->actingAs($user)
            ->delete(route('admin.media.destroy', $media))
            ->assertForbidden();

        $this->actingAs($user)
            ->patch(route('admin.homepage-sections.update'), [
                'sections' => [[
                    'id' => $section->id,
                    'label' => 'Should Fail',
                    'position' => 1,
                    'is_enabled' => true,
                ]],
            ])
            ->assertForbidden();

        $this->actingAs($user)
            ->post(route('admin.recommended-projects.store'), [
                'title' => 'Forbidden Project',
                'description' => 'Should never be created.',
                'ratio' => '16:9',
                'display_priority' => 5,
                'is_active' => '1',
            ])
            ->assertForbidden();

        $this->actingAs($user)
            ->put(route('admin.recommended-projects.update', $recommendedProject), [
                'title' => 'Should Fail Update',
                'description' => 'Still forbidden.',
                'ratio' => '16:9',
                'display_priority' => 50,
            ])
            ->assertForbidden();

        $this->actingAs($user)
            ->patch(route('admin.recommended-projects.toggle-active', $recommendedProject))
            ->assertForbidden();

        $this->actingAs($user)
            ->patch(route('admin.recommended-projects.show-now', $recommendedProject))
            ->assertForbidden();

        $this->actingAs($user)
            ->delete(route('admin.recommended-projects.destroy', $recommendedProject))
            ->assertForbidden();

        $recommendedProject->refresh();

        $this->assertSame('Locked Recommendation', $recommendedProject->title);
        $this->assertTrue($recommendedProject->is_active);
        $this->assertDatabaseCount('recommended_projects', 1);
    }
}