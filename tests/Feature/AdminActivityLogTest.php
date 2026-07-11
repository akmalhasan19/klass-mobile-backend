<?php

namespace Tests\Feature;

use App\Models\ActivityLog;
use App\Models\Topic;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminActivityLogTest extends TestCase
{
    use RefreshDatabase;

    public function test_admin_auth_flow_creates_login_and_logout_activity_logs(): void
    {
        $admin = User::factory()->admin()->create([
            'email' => 'admin.audit@klass.id',
            'password' => bcrypt('secret123'),
        ]);

        $this->post(route('admin.login.post'), [
            'email' => 'admin.audit@klass.id',
            'password' => 'secret123',
        ])->assertRedirect(route('admin.dashboard'));

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'admin_login',
            'subject_type' => User::class,
            'subject_id' => $admin->id,
        ]);

        $this->post(route('admin.logout'))
            ->assertRedirect(route('admin.login'));

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'admin_logout',
            'subject_type' => User::class,
            'subject_id' => $admin->id,
        ]);
    }

    public function test_activity_log_index_can_filter_by_actor_action_subject_and_date(): void
    {
        $admin = User::factory()->admin()->create([
            'name' => 'Primary Admin',
            'email' => 'primary@klass.id',
        ]);

        $secondaryAdmin = User::factory()->admin()->create([
            'name' => 'Secondary Admin',
            'email' => 'secondary@klass.id',
        ]);

        $topic = Topic::create([
            'title' => 'Audited Topic',
            'teacher_id' => 'teacher-audit',
            'order' => 1,
        ]);

        ActivityLog::create([
            'actor_id' => $admin->id,
            'action' => 'target_action',
            'subject_type' => Topic::class,
            'subject_id' => $topic->id,
            'metadata' => ['scope' => 'target'],
        ]);

        ActivityLog::create([
            'actor_id' => $secondaryAdmin->id,
            'action' => 'other_action',
            'subject_type' => User::class,
            'subject_id' => $secondaryAdmin->id,
            'metadata' => ['scope' => 'other'],
            'created_at' => now()->subDays(10),
            'updated_at' => now()->subDays(10),
        ]);

        $this->actingAs($admin)
            ->get(route('admin.activity-logs.index', [
                'action' => 'target_action',
                'actor_id' => $admin->id,
                'subject_type' => Topic::class,
                'date_from' => now()->toDateString(),
                'date_to' => now()->toDateString(),
            ]))
            ->assertOk()
            ->assertViewHas('logs', function ($logs) use ($admin, $topic) {
                return $logs->count() === 1
                    && $logs->first()->actor_id === $admin->id
                    && $logs->first()->action === 'target_action'
                    && $logs->first()->subject_type === Topic::class
                    && $logs->first()->subject_id === $topic->id;
            })
            ->assertSeeText('Primary Admin')
            ->assertSeeText('target_action')
            ->assertSeeText('Topic');
    }
}