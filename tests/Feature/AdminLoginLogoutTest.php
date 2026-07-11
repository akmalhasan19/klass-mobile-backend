<?php

namespace Tests\Feature;

use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminLoginLogoutTest extends TestCase
{
    use RefreshDatabase;

    // -----------------------------------------------------------------------
    // Login page
    // -----------------------------------------------------------------------

    public function test_admin_login_page_is_accessible_without_authentication(): void
    {
        $this->get(route('admin.login'))
            ->assertOk()
            ->assertSeeText('Klass Admin');
    }

    public function test_authenticated_admin_is_redirected_from_login_page(): void
    {
        $admin = User::factory()->admin()->create();

        $this->actingAs($admin)
            ->get(route('admin.login'))
            ->assertRedirect(route('admin.dashboard'));
    }

    // -----------------------------------------------------------------------
    // Login POST
    // -----------------------------------------------------------------------

    public function test_admin_can_login_with_valid_credentials(): void
    {
        $admin = User::factory()->admin()->create([
            'email'    => 'admin@klass.id',
            'password' => bcrypt('secret123'),
        ]);

        $response = $this->post(route('admin.login.post'), [
            'email'    => 'admin@klass.id',
            'password' => 'secret123',
        ]);

        $response->assertRedirect(route('admin.dashboard'));
        $this->assertAuthenticatedAs($admin);
    }

    public function test_non_admin_user_cannot_login_to_admin_panel(): void
    {
        User::factory()->create([
            'email'    => 'user@klass.id',
            'password' => bcrypt('secret123'),
        ]);

        $response = $this->post(route('admin.login.post'), [
            'email'    => 'user@klass.id',
            'password' => 'secret123',
        ]);

        // Harus dikembalikan ke form login dengan error
        $response->assertRedirect();
        $this->assertGuest();
    }

    public function test_login_fails_with_wrong_password(): void
    {
        User::factory()->admin()->create([
            'email'    => 'admin@klass.id',
            'password' => bcrypt('correct-password'),
        ]);

        $this->post(route('admin.login.post'), [
            'email'    => 'admin@klass.id',
            'password' => 'wrong-password',
        ])->assertRedirect();

        $this->assertGuest();
    }

    public function test_login_validates_required_fields(): void
    {
        $this->post(route('admin.login.post'), [])
            ->assertSessionHasErrors(['email', 'password']);
    }

    public function test_login_validates_email_format(): void
    {
        $this->post(route('admin.login.post'), [
            'email'    => 'not-a-valid-email',
            'password' => 'password123',
        ])->assertSessionHasErrors(['email']);
    }

    // -----------------------------------------------------------------------
    // Dashboard access after login
    // -----------------------------------------------------------------------

    public function test_unauthenticated_access_to_dashboard_redirects_to_login(): void
    {
        $this->get(route('admin.dashboard'))
            ->assertRedirect(route('admin.login'));
    }

    public function test_authenticated_admin_can_access_dashboard(): void
    {
        $admin = User::factory()->admin()->create();

        $this->actingAs($admin)
            ->get(route('admin.dashboard'))
            ->assertOk();
    }

    // -----------------------------------------------------------------------
    // Logout
    // -----------------------------------------------------------------------

    public function test_admin_can_logout(): void
    {
        $admin = User::factory()->admin()->create();

        $this->actingAs($admin)
            ->post(route('admin.logout'))
            ->assertRedirect(route('admin.login'));

        $this->assertGuest();
    }

    public function test_logout_without_authentication_redirects_safely(): void
    {
        // POST /admin/logout tanpa autentikasi:
        // EnsureUserIsAdmin akan redirect ke admin.login (bukan crash)
        // Kita bypass CSRF agar test ini fokus pada auth behavior
        $this->withoutMiddleware(\Illuminate\Foundation\Http\Middleware\VerifyCsrfToken::class)
            ->post(route('admin.logout'))
            ->assertRedirect(route('admin.login'));
    }
}
