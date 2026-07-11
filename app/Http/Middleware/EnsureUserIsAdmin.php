<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

class EnsureUserIsAdmin
{
    public function handle(Request $request, Closure $next): Response
    {
        $user = $request->user();

        if ($user?->isAdmin()) {
            return $next($request);
        }

        // API / JSON requests tetap mendapat respons JSON
        if ($request->is('api/*') || $request->expectsJson()) {
            return new JsonResponse([
                'success' => false,
                'message' => 'Akses admin diperlukan.',
            ], $user ? 403 : 401);
        }

        // Web request: redirect ke halaman login admin
        if (! $user) {
            return redirect()->route('admin.login')
                ->with('error', 'Silakan login terlebih dahulu untuk mengakses panel admin.');
        }

        // Sudah login tapi bukan admin
        abort(403, 'Akses admin diperlukan.');
    }
}