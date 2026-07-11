<?php

return [

    /*
    |--------------------------------------------------------------------------
    | Cross-Origin Resource Sharing (CORS) Configuration
    |--------------------------------------------------------------------------
    |
    | Settings for cross-origin requests. The paths option controls which
    | routes receive CORS headers. Origins are read from the environment
    | as a comma-separated list — set CORS_ALLOWED_ORIGINS in HF Secrets
    | for production.
    |
    | NOTE: When supports_credentials is true, the allowed_origins cannot
    | contain '*'. The fallback below uses concrete localhost addresses
    | to remain compatible with credentialled requests during development.
    |
    */

    'paths' => ['api/*', 'sanctum/csrf-cookie'],

    'allowed_methods' => ['*'],

    'allowed_origins' => array_filter(explode(',', env('CORS_ALLOWED_ORIGINS', 'http://localhost:3000,http://localhost:8000,http://localhost:5173'))),

    'allowed_origins_patterns' => [],

    'allowed_headers' => ['*'],

    'exposed_headers' => [],

    'max_age' => env('CORS_PREFLIGHT_MAX_AGE', 86400),

    'supports_credentials' => env('CORS_SUPPORTS_CREDENTIALS', true),

];
