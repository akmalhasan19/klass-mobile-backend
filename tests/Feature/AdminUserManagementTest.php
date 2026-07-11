<?php

namespace Tests\Feature;

use App\Models\ActivityLog;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminUserManagementTest extends TestCase
{
    use RefreshDatabase;

    public function test_admin_can_search_user_list_and_view_user_details(): void
    {
        $admin = User::factory()->admin()->create();

        $targetUser = User::factory()->create([
            'name' => 'Target Student',
            'email' => 'target.student@klass.id',
        ]);

        User::factory()->create([
            'name' => 'Hidden User',
            'email' => 'hidden.user@klass.id',
        ]);

        $this->actingAs($admin)
            ->get(route('admin.users.index', ['search' => 'target.student']))
            ->assertOk()
            ->assertSeeText('Target Student')
            ->assertSeeText('target.student@klass.id')
            ->assertDontSeeText('hidden.user@klass.id');

        $this->actingAs($admin)
            ->get(route('admin.users.show', $targetUser))
            ->assertOk()
            ->assertSeeText('Target Student')
            ->assertSeeText('Kontrol Hak Akses')
            ->assertSeeText(User::ROLE_USER);
    }

    public function test_admin_can_update_user_role_and_log_the_change(): void
    {
        $admin = User::factory()->admin()->create();
        $user = User::factory()->create([
            'role' => User::ROLE_USER,
        ]);

        $this->actingAs($admin)
            ->patch(route('admin.users.update-role', $user), [
                'role' => User::ROLE_ADMIN,
            ])
            ->assertRedirect();

        $this->assertDatabaseHas('users', [
            'id' => $user->id,
            'role' => User::ROLE_ADMIN,
        ]);

        $this->assertDatabaseHas('activity_logs', [
            'actor_id' => $admin->id,
            'action' => 'update_role',
            'subject_type' => User::class,
            'subject_id' => $user->id,
        ]);

        $log = ActivityLog::where('action', 'update_role')->first();

        $this->assertSame(User::ROLE_USER, $log->metadata['old_role']);
        $this->assertSame(User::ROLE_ADMIN, $log->metadata['new_role']);
    }
}