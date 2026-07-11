<?php

namespace Tests\Feature;

use App\Models\ActivityLog;
use App\Models\RecommendedProject;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Tests\TestCase;

class AdminRecommendedProjectManagementTest extends TestCase
{
    use RefreshDatabase;

    public function test_admin_can_create_update_toggle_and_delete_recommended_project(): void
    {
        $admin = User::factory()->admin()->create();

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->post(route('admin.recommended-projects.store'), [
                'title' => 'Klass CRM Showcase',
                'description' => 'Curated admin project for the homepage feed.',
                'ratio' => '16:9',
                'project_type' => 'web',
                'tags' => 'Laravel, CRM, Dashboard',
                'modules' => 'Auth, Reports',
                'display_priority' => 42,
                'is_active' => '1',
            ])
            ->assertRedirect(route('admin.homepage-sections.index'));

        $project = RecommendedProject::query()->latest('id')->firstOrFail();

        $this->assertSame('Klass CRM Showcase', $project->title);
        $this->assertSame(['Laravel', 'CRM', 'Dashboard'], $project->tags);
        $this->assertSame(['Auth', 'Reports'], $project->modules);
        $this->assertSame(42, $project->display_priority);
        $this->assertTrue($project->is_active);
        $this->assertSame($admin->id, $project->created_by);
        $this->assertSame($admin->id, $project->updated_by);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'create_recommended_project',
            'subject_type' => RecommendedProject::class,
            'subject_id' => $project->id,
        ]);

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->put(route('admin.recommended-projects.update', $project), [
                'title' => 'Klass CRM Showcase v2',
                'description' => 'Updated description for admin curation.',
                'ratio' => '4:3',
                'project_type' => 'mobile',
                'tags' => 'Flutter, Analytics',
                'modules' => 'Feed, Insights, Notification',
                'display_priority' => 77,
            ])
            ->assertRedirect(route('admin.homepage-sections.index'));

        $project->refresh();

        $this->assertSame('Klass CRM Showcase v2', $project->title);
        $this->assertSame('4:3', $project->ratio);
        $this->assertSame('mobile', $project->project_type);
        $this->assertSame(['Flutter', 'Analytics'], $project->tags);
        $this->assertSame(['Feed', 'Insights', 'Notification'], $project->modules);
        $this->assertSame(77, $project->display_priority);
        $this->assertFalse($project->is_active);
        $this->assertSame($admin->id, $project->updated_by);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'update_recommended_project',
            'subject_type' => RecommendedProject::class,
            'subject_id' => $project->id,
        ]);

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->patch(route('admin.recommended-projects.toggle-active', $project))
            ->assertRedirect(route('admin.homepage-sections.index'));

        $project->refresh();

        $this->assertTrue($project->is_active);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'toggle_active_recommended_project',
            'subject_type' => RecommendedProject::class,
            'subject_id' => $project->id,
        ]);

        $deletedProjectId = $project->id;

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->delete(route('admin.recommended-projects.destroy', $project))
            ->assertRedirect(route('admin.homepage-sections.index'));

        $this->assertDatabaseMissing('recommended_projects', [
            'id' => $deletedProjectId,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'delete_recommended_project',
            'subject_type' => RecommendedProject::class,
            'subject_id' => $deletedProjectId,
        ]);

        $this->assertSame(4, ActivityLog::query()->count());
    }

    public function test_admin_can_upload_thumbnail_when_creating_recommended_project(): void
    {
        Storage::fake('supabase');

        config()->set('filesystems.disks.supabase.endpoint', 'https://storage.example.test');
        config()->set('filesystems.disks.supabase.bucket', 'klass-storage-test');

        $admin = User::factory()->admin()->create();

        $this->actingAs($admin)
            ->from(route('admin.homepage-sections.index'))
            ->post(route('admin.recommended-projects.store'), [
                'title' => 'Uploaded Thumbnail Project',
                'description' => 'Project with uploaded thumbnail.',
                'ratio' => '16:9',
                'thumbnail' => UploadedFile::fake()->image('recommended-project.png', 1280, 720),
                'display_priority' => 10,
                'is_active' => '1',
            ])
            ->assertRedirect(route('admin.homepage-sections.index'));

        $project = RecommendedProject::query()->latest('id')->firstOrFail();
        $storedFiles = Storage::disk('supabase')->allFiles('gallery');

        $this->assertNotNull($project->thumbnail_url);
        $this->assertStringContainsString('/gallery/', $project->thumbnail_url);
        $this->assertCount(1, $storedFiles);
        $this->assertStringStartsWith('gallery/', $storedFiles[0]);
        $this->assertStringEndsWith('.png', $storedFiles[0]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'create_recommended_project',
            'subject_type' => RecommendedProject::class,
            'subject_id' => $project->id,
        ]);
    }
}