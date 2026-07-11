<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\GetSecurityQuestionRequest;
use App\Http\Requests\LoginRequest;
use App\Http\Requests\RegisterRequest;
use App\Http\Requests\ResetPasswordRequest;
use App\Http\Resources\UserResource;
use App\Http\Traits\ApiResponseTrait;
use App\Models\ActivityLog;
use App\Models\User;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Hash;

/**
 * AuthController
 *
 * Menangani registrasi, login, logout, dan info user saat ini.
 *
 * POST   /api/auth/register  — Registrasi user baru
 * POST   /api/auth/login     — Login, return Sanctum token
 * POST   /api/auth/logout    — Logout, revoke token
 * GET    /api/auth/me        — Data user yang sedang login
 */
class AuthController extends Controller
{
    use ApiResponseTrait;

    /**
     * Registrasi user baru.
     */
    public function register(RegisterRequest $request): JsonResponse
    {
        $role = $request->input('role', User::ROLE_TEACHER);

        // Validate role is one of the allowed values
        if (!in_array($role, [User::ROLE_TEACHER, User::ROLE_FREELANCER], true)) {
            return $this->error('Role tidak valid. Pilih teacher atau freelancer.', 422);
        }

        $user = User::create([
            'name' => $request->name,
            'email' => $request->email,
            'password' => Hash::make($request->password),
            'primary_subject_id' => $request->integer('primary_subject_id') ?: null,
            'role' => $role,
        ]);

        $token = $user->createToken('auth-token')->plainTextToken;

        return $this->created([
            'user' => new UserResource($user),
            'token' => $token,
        ], 'Registrasi berhasil.');
    }

    /**
     * Login user.
     */
    public function login(LoginRequest $request): JsonResponse
    {
        $email = $request->input('email');

        if (!Auth::attempt($request->only('email', 'password'))) {
            ActivityLog::create([
                'actor_id' => null,
                'action' => 'failed_login_attempt',
                'metadata' => [
                    'email' => $email,
                    'ip' => $request->ip(),
                    'user_agent' => $request->userAgent(),
                    'attempted_at' => now()->toIso8601String(),
                ],
            ]);

            return $this->error('Email atau password salah.', 401);
        }

        /** @var User $user */
        $user = Auth::user();
        $token = $user->createToken('auth-token')->plainTextToken;

        return $this->success([
            'user' => new UserResource($user),
            'token' => $token,
        ], 'Login berhasil.');
    }

    /**
     * Logout — revoke current token.
     */
    public function logout(Request $request): JsonResponse
    {
        $request->user()->currentAccessToken()->delete();

        return $this->success(null, 'Logout berhasil.');
    }

    /**
     * Info user yang sedang login.
     */
    public function me(Request $request): JsonResponse
    {
        return $this->success(
            new UserResource($request->user()),
            'Data user berhasil diambil.',
        );
    }

    /**
     * Refresh token — revoke current and issue new one.
     */
    public function refresh(Request $request): JsonResponse
    {
        $user = $request->user();

        $currentToken = $user->currentAccessToken();
        if ($currentToken !== null && $currentToken->exists) {
            $currentToken->delete();
        }

        $token = $user->createToken('auth-token')->plainTextToken;

        return $this->success([
            'token' => $token,
        ], 'Token berhasil diperbarui.');
    }

    public function getSecurityQuestion(GetSecurityQuestionRequest $request): JsonResponse
    {
        $user = User::where('email', $request->email)->first();

        if (!$user || !$user->security_question) {
            return $this->error('User tidak ditemukan atau belum mengatur pertanyaan keamanan.', 404);
        }

        return $this->success([
            'security_question' => $user->security_question,
        ], 'Pertanyaan keamanan berhasil diambil.');
    }

    public function verifyAndResetPassword(ResetPasswordRequest $request): JsonResponse
    {
        $user = User::where('email', $request->email)->first();

        if (!$user || !$user->security_answer) {
            return $this->error('User tidak ditemukan atau belum mengatur pertanyaan keamanan.', 404);
        }

        if (!Hash::check($request->security_answer, $user->security_answer)) {
            return $this->error('Jawaban keamanan salah.', 403);
        }

        $user->password = Hash::make($request->new_password);
        $user->save();

        return $this->success(null, 'Password berhasil diubah.');
    }
}
