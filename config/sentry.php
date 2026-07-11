<?php

return [

    'dsn' => env('SENTRY_LARAVEL_DSN'),

    'release' => env('SENTRY_RELEASE'),

    'environment' => env('SENTRY_ENVIRONMENT', env('APP_ENV', 'production')),

    'breadcrumbs' => [
        'sql_queries' => true,
        'sql_bindings' => false,
        'queue_info' => true,
        'command_info' => true,
    ],

    'tracing' => [
        'enabled' => env('SENTRY_TRACING_ENABLED', true),
        'sql_queries' => true,
        'sql_origin' => true,
        'queue_info' => true,
        'redis_commands' => false,
    ],

    'profiles_sample_rate' => env('SENTRY_PROFILES_SAMPLE_RATE', 0.2),

    'traces_sample_rate' => env('SENTRY_TRACES_SAMPLE_RATE', 0.5),

    'send_default_pii' => false,

    'max_breadcrumbs' => 50,

    'ignore_exceptions' => [
        \Illuminate\Validation\ValidationException::class,
        \Illuminate\Auth\AuthenticationException::class,
        \Symfony\Component\HttpKernel\Exception\NotFoundHttpException::class,
    ],

];
