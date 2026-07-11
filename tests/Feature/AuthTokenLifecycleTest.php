<?php

namespace Tests\Feature;

use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Cache;
use Laravel\Sanctum\Sanctum;
use Tests\TestCase;

class AuthTokenLifecycleTest extends TestCase
{
    use RefreshDatabase;

    protected function setUp(): void
    {
        parent::setUp();
        Cache::flush();
    }

    public function test_refresh_returns_new_token(): void
    {
        $user = User::factory()->create();
        Sanctum::actingAs($user);

        $response = $this->postJson('/api/v1/auth/refresh');

        $response
            ->assertOk()
            ->assertJsonPath('success', true)
            ->assertJsonStructure(['data' => ['token']]);
    }

    public function test_refresh_creates_new_token_in_database(): void
    {
        $user = User::factory()->create();
        Sanctum::actingAs($user);

        $this->postJson('/api/v1/auth/refresh')->assertOk();

        $this->assertDatabaseCount('personal_access_tokens', 1);
    }

    public function test_revoked_token_rejected(): void
    {
        $user = User::factory()->create();
        $token = $user->createToken('test-token');
        $plainText = $token->plainTextToken;
        $token->accessToken->delete();

        $this->withToken($plainText)
            ->getJson('/api/v1/auth/me')
            ->assertUnauthorized();
    }

    public function test_expired_token_rejected(): void
    {
        $user = User::factory()->create();
        $token = $user->createToken('test-token');
        $token->accessToken->forceFill(['expires_at' => now()->subDay()])->save();

        $this->withToken($token->plainTextToken)
            ->getJson('/api/v1/auth/me')
            ->assertUnauthorized();
    }

    public function test_rate_limit_triggered_on_login(): void
    {
        $user = User::factory()->create();

        for ($i = 0; $i < 6; $i++) {
            $response = $this->postJson('/api/v1/auth/login', [
                'email' => $user->email,
                'password' => 'wrong-password',
            ]);

            if ($i < 5) {
                $response->assertStatus(401);
            } else {
                $response->assertStatus(429);
            }
        }
    }

    public function test_rate_limit_triggered_on_register(): void
    {
        for ($i = 0; $i < 4; $i++) {
            $response = $this->postJson('/api/v1/auth/register', [
                'name' => "User {$i}",
                'email' => "user{$i}@test.com",
                'password' => 'password',
                'password_confirmation' => 'password',
            ]);

            if ($i < 3) {
                $response->assertStatus(201);
            } else {
                $response->assertStatus(429);
            }
        }
    }

    public function test_rate_limit_triggered_on_reset_password(): void
    {
        for ($i = 0; $i < 4; $i++) {
            $response = $this->postJson('/api/v1/auth/verify-and-reset-password', [
                'email' => "user{$i}@test.com",
                'security_answer' => 'answer',
                'new_password' => 'new-password',
            ]);

            if ($i < 3) {
                $response->assertStatus(404);
            } else {
                $response->assertStatus(429);
            }
        }
    }
}
