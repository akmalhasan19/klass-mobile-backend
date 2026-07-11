<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Log;
use Symfony\Component\HttpFoundation\Response;

/**
 * StructuredApiLogger
 *
 * Middleware untuk logging terstruktur pada semua request API.
 * Log disimpan ke channel 'api' (atau default) dalam format JSON
 * yang mudah di-parse oleh monitoring tools (Datadog, ELK, dll).
 *
 * Data yang di-log:
 * - Request: method, path, query params, IP, user agent
 * - Response: status code, duration (ms)
 * - Error: jika status >= 400, response body di-log sebagai context
 */
class StructuredApiLogger
{
    /**
     * Priority endpoints yang mendapat log lebih detail.
     */
    private const PRIORITY_ENDPOINTS = [
        'api/auth/login',
        'api/auth/register',
        'api/auth/me',
        'api/topics',
        'api/marketplace-tasks',
        'api/gallery',
        'api/user/avatar',
        'api/contents',
    ];

    public function handle(Request $request, Closure $next): Response
    {
        $startTime = microtime(true);

        /** @var Response $response */
        $response = $next($request);

        $durationMs = round((microtime(true) - $startTime) * 1000, 2);
        $path = $request->path();
        $statusCode = $response->getStatusCode();

        $logData = [
            'method'      => $request->method(),
            'path'        => $path,
            'status'      => $statusCode,
            'duration_ms' => $durationMs,
            'ip'          => $request->ip(),
            'user_id'     => $request->user()?->id,
            'user_agent'  => substr($request->userAgent() ?? '', 0, 120),
            'query'       => $request->query() ?: null,
        ];

        // Determine log level based on status code
        if ($statusCode >= 500) {
            $logData['response_body'] = $this->truncateBody($response);
            Log::channel('api')->error('[API] Server Error', $logData);
        } elseif ($statusCode >= 400) {
            $logData['response_body'] = $this->truncateBody($response);
            Log::channel('api')->warning('[API] Client Error', $logData);
        } elseif ($this->isPriorityEndpoint($path)) {
            Log::channel('api')->info('[API] Request', $logData);
        } else {
            // Non-priority successful requests: debug level
            Log::channel('api')->debug('[API] Request', $logData);
        }

        // Slow request detection (> 2 seconds)
        if ($durationMs > 2000) {
            Log::channel('api')->warning('[API] Slow Request Detected', [
                'path'        => $path,
                'duration_ms' => $durationMs,
                'method'      => $request->method(),
            ]);
        }

        return $response;
    }

    private function isPriorityEndpoint(string $path): bool
    {
        foreach (self::PRIORITY_ENDPOINTS as $endpoint) {
            if (str_starts_with($path, $endpoint)) {
                return true;
            }
        }

        return false;
    }

    private function truncateBody(Response $response): ?string
    {
        $body = $response->getContent();
        if ($body === false || $body === '') {
            return null;
        }

        return mb_substr($body, 0, 500);
    }
}
