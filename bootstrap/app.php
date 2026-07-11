<?php

use App\Http\Middleware\EnsureUserIsAdmin;
use App\Http\Middleware\EnsureUserIsFreelancer;
use App\Http\Middleware\EnsureUserIsTeacher;
use App\Http\Middleware\StructuredApiLogger;
use App\MediaGeneration\MediaGenerationApiException;
use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationErrorCode;
use Illuminate\Auth\AuthenticationException;
use Illuminate\Database\Eloquent\ModelNotFoundException;
use Illuminate\Foundation\Application;
use Illuminate\Foundation\Configuration\Exceptions;
use Illuminate\Foundation\Configuration\Middleware;
use Illuminate\Http\Exceptions\ThrottleRequestsException;
use Illuminate\Http\Middleware\HandleCors;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Log;
use Illuminate\Validation\ValidationException;
use Symfony\Component\HttpKernel\Exception\NotFoundHttpException;

return Application::configure(basePath: dirname(__DIR__))
    ->withRouting(
        web: __DIR__.'/../routes/web.php',
        api: __DIR__.'/../routes/api.php',
        commands: __DIR__.'/../routes/console.php',
        health: '/up',
    )
    ->withMiddleware(function (Middleware $middleware): void {
        $middleware->trustProxies(at: '*');

        // CORS headers for api/* and sanctum/csrf-cookie (see config/cors.php)
        $middleware->prepend(HandleCors::class);

        $middleware->alias([
            'admin' => EnsureUserIsAdmin::class,
            'teacher' => EnsureUserIsTeacher::class,
            'freelancer' => EnsureUserIsFreelancer::class,
        ]);

        // Structured API logging for all API routes
        $middleware->api(append: [
            StructuredApiLogger::class,
        ]);
    })
    ->withExceptions(function (Exceptions $exceptions): void {
        /*
        |----------------------------------------------------------------------
        | API Exception Rendering — JSON terstruktur
        |----------------------------------------------------------------------
        | Semua exception dari request API (/api/*) di-render
        | sebagai JSON dengan format konsisten:
        | { "success": false, "message": "...", "errors": { ... } }
        */

        $exceptions->renderable(function (MediaGenerationApiException $e, Request $request) {
            if ($request->is('api/*') || $request->expectsJson()) {
                $response = [
                    'success' => false,
                    'message' => $e->getMessage(),
                    'error' => MediaGenerationErrorCode::toClientPayload($e->errorCode()),
                    'timestamp' => now()->toIso8601String(),
                ];

                if ($e->errors() !== []) {
                    $response['errors'] = $e->errors();
                }

                return response()->json($response, $e->statusCode());
            }
        });

        $exceptions->renderable(function (MediaGenerationContractException $e, Request $request) {
            if ($request->is('api/*') || $request->expectsJson()) {
                $errorCode = $e->errorCode();

                return response()->json([
                    'success' => false,
                    'message' => MediaGenerationErrorCode::clientMessage($errorCode),
                    'error' => MediaGenerationErrorCode::toClientPayload($errorCode),
                    'timestamp' => now()->toIso8601String(),
                ], MediaGenerationErrorCode::httpStatus($errorCode));
            }
        });

        // Validation errors → 422
        $exceptions->renderable(function (ValidationException $e, Request $request) {
            if ($request->is('api/*') || $request->expectsJson()) {
                return response()->json([
                    'success' => false,
                    'message' => 'Validasi gagal.',
                    'errors' => $e->errors(),
                    'error' => ['code' => 'VALIDATION_FAILED'],
                    'timestamp' => now()->toIso8601String(),
                ], 422);
            }
        });

        // Model not found → 404
        $exceptions->renderable(function (ModelNotFoundException $e, Request $request) {
            if ($request->is('api/*') || $request->expectsJson()) {
                $model = class_basename($e->getModel());
                return response()->json([
                    'success' => false,
                    'message' => "{$model} tidak ditemukan.",
                    'error' => ['code' => 'NOT_FOUND'],
                    'timestamp' => now()->toIso8601String(),
                ], 404);
            }
        });

        // Route not found → 404
        $exceptions->renderable(function (NotFoundHttpException $e, Request $request) {
            if ($request->is('api/*') || $request->expectsJson()) {
                return response()->json([
                    'success' => false,
                    'message' => 'Endpoint tidak ditemukan.',
                    'error' => ['code' => 'NOT_FOUND'],
                    'timestamp' => now()->toIso8601String(),
                ], 404);
            }
        });

        // Authentication failures → 401
        $exceptions->renderable(function (AuthenticationException $e, Request $request) {
            if ($request->is('api/*') || $request->expectsJson()) {
                return response()->json([
                    'success' => false,
                    'message' => 'Tidak memiliki akses. Silakan login terlebih dahulu.',
                    'error' => ['code' => 'UNAUTHENTICATED'],
                    'timestamp' => now()->toIso8601String(),
                ], 401);
            }
        });

        // Rate limit exceeded → 429
        $exceptions->renderable(function (ThrottleRequestsException $e, Request $request) {
            if ($request->is('api/*') || $request->expectsJson()) {
                return response()->json([
                    'success' => false,
                    'message' => 'Terlalu banyak permintaan. Silakan coba lagi nanti.',
                    'error' => ['code' => 'RATE_LIMITED'],
                    'timestamp' => now()->toIso8601String(),
                ], 429);
            }
        });

        // Catch-all: unexpected exceptions → 500
        $exceptions->renderable(function (\Throwable $e, Request $request) {
            if ($request->is('api/*') || $request->expectsJson()) {
                Log::error('[API] Unhandled Exception', [
                    'exception' => get_class($e),
                    'message'   => $e->getMessage(),
                    'file'      => $e->getFile(),
                    'line'      => $e->getLine(),
                    'path'      => $request->path(),
                    'method'    => $request->method(),
                ]);

                return response()->json([
                    'success' => false,
                    'message' => 'Terjadi kesalahan pada server.',
                    'error' => ['code' => 'SERVER_ERROR'],
                    'timestamp' => now()->toIso8601String(),
                ], 500);
            }
        });
    })->create();
