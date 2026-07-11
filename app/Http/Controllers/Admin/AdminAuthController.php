<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use Illuminate\Http\RedirectResponse;
use App\Models\ActivityLog;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\View\View;

class AdminAuthController extends Controller
{
    /**
     * Tampilkan form login admin.
     */
    public function showLogin(): View|RedirectResponse
    {
        if (Auth::check() && Auth::user()->isAdmin()) {
            return redirect()->route('admin.dashboard');
        }

        return view('admin.auth.login');
    }

    /**
     * Proses login admin.
     */
    public function login(Request $request): RedirectResponse
    {
        $credentials = $request->validate([
            'email'    => ['required', 'email'],
            'password' => ['required'],
        ]);

        if (Auth::attempt($credentials, $request->boolean('remember'))) {
            $user = Auth::user();

            if (! $user->isAdmin()) {
                Auth::logout();
                $request->session()->invalidate();
                $request->session()->regenerateToken();

                return back()->withErrors([
                    'email' => 'Akun ini tidak memiliki akses admin.',
                ])->onlyInput('email');
            }

            $request->session()->regenerate();

            ActivityLog::create([
                'actor_id'     => $user->id,
                'action'       => 'admin_login',
                'subject_type' => get_class($user),
                'subject_id'   => $user->id,
                'metadata'     => ['ip' => $request->ip(), 'user_agent' => $request->userAgent()],
            ]);

            return redirect()->intended(route('admin.dashboard'));
        }

        return back()->withErrors([
            'email' => 'Email atau password tidak sesuai.',
        ])->onlyInput('email');
    }

    /**
     * Logout admin.
     */
    public function logout(Request $request): RedirectResponse
    {
        if (Auth::check()) {
            ActivityLog::create([
                'actor_id'     => Auth::id(),
                'action'       => 'admin_logout',
                'subject_type' => get_class(Auth::user()),
                'subject_id'   => Auth::id(),
                'metadata'     => ['ip' => $request->ip(), 'user_agent' => $request->userAgent()],
            ]);
        }

        Auth::logout();
        $request->session()->invalidate();
        $request->session()->regenerateToken();

        return redirect()->route('admin.login')
            ->with('success', 'Anda berhasil logout dari panel admin.');
    }
}
