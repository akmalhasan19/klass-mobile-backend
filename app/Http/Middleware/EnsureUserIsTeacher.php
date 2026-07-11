<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

/**
 * EnsureUserIsTeacher
 *
 * Memvalidasi bahwa user yang sedang login memiliki role teacher (atau admin).
 * Digunakan untuk melindungi route yang hanya boleh diakses oleh teacher.
 */
class EnsureUserIsTeacher
{
    public function handle(Request $request, Closure $next): Response
    {
        $user = $request->user();

        if ($user?->isAdmin() || $user?->isTeacher()) {
            return $next($request);
        }

        if ($request->is('api/*') || $request->expectsJson()) {
            return new JsonResponse([
                'success' => false,
                'message' => 'Akses khusus teacher. Anda tidak memiliki izin untuk mengakses fitur ini.',
            ], $user ? 403 : 401);
        }

        abort(403, 'Akses khusus teacher.');
    }
}
