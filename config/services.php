<?php

return [

    /*
    |--------------------------------------------------------------------------
    | Third Party Services
    |--------------------------------------------------------------------------
    |
    | This file is for storing the credentials for third party services such
    | as Mailgun, Postmark, AWS and more. This file provides the de facto
    | location for this type of information, allowing packages to have
    | a conventional file to locate the various service credentials.
    |
    */

    'postmark' => [
        'key' => env('POSTMARK_API_KEY'),
    ],

    'resend' => [
        'key' => env('RESEND_API_KEY'),
    ],

    'ses' => [
        'key' => env('AWS_ACCESS_KEY_ID'),
        'secret' => env('AWS_SECRET_ACCESS_KEY'),
        'region' => env('AWS_DEFAULT_REGION', 'us-east-1'),
    ],

    'slack' => [
        'notifications' => [
            'bot_user_oauth_token' => env('SLACK_BOT_USER_OAUTH_TOKEN'),
            'channel' => env('SLACK_BOT_USER_DEFAULT_CHANNEL'),
        ],
    ],

    'media_generation' => [
        'llm_adapter' => [
            'base_url' => env(
                'MEDIA_GENERATION_LLM_ADAPTER_BASE_URL',
                env('MEDIA_GENERATION_INTERPRETER_BASE_URL', env('MEDIA_GENERATION_DELIVERY_BASE_URL')),
            ),
            'health_path' => env('MEDIA_GENERATION_LLM_ADAPTER_HEALTH_PATH', '/v1/health'),
            'shared_secret' => env('MEDIA_GENERATION_LLM_ADAPTER_SHARED_SECRET'),
            'request_max_age_seconds' => (int) env('MEDIA_GENERATION_LLM_ADAPTER_REQUEST_MAX_AGE_SECONDS', 300),
            'clock_skew_seconds' => (int) env('MEDIA_GENERATION_LLM_ADAPTER_CLOCK_SKEW_SECONDS', 30),
            'service_name' => env('MEDIA_GENERATION_LLM_ADAPTER_SERVICE_NAME', 'llm-adapter-service'),
            'service_version' => env('MEDIA_GENERATION_LLM_ADAPTER_SERVICE_VERSION'),
        ],
        'interpreter' => [
            'base_url' => env(
                'MEDIA_GENERATION_LLM_ADAPTER_BASE_URL',
                env('MEDIA_GENERATION_INTERPRETER_BASE_URL'),
            ),
            'path' => env('MEDIA_GENERATION_INTERPRETER_PATH', '/v1/interpret'),
            'provider' => env('MEDIA_GENERATION_INTERPRETER_PROVIDER', 'llm-adapter'),
            'model' => env('MEDIA_GENERATION_INTERPRETER_MODEL', 'adapter-managed'),
            'timeout_seconds' => (float) env('MEDIA_GENERATION_INTERPRETER_TIMEOUT_SECONDS', 30),
            'connect_timeout_seconds' => (float) env('MEDIA_GENERATION_INTERPRETER_CONNECT_TIMEOUT_SECONDS', 10),
            'retry_attempts' => (int) env('MEDIA_GENERATION_INTERPRETER_RETRY_ATTEMPTS', 2),
            'retry_sleep_milliseconds' => (int) env('MEDIA_GENERATION_INTERPRETER_RETRY_SLEEP_MILLISECONDS', 250),
        ],
        'drafting' => [
            'base_url' => env(
                'MEDIA_GENERATION_LLM_ADAPTER_BASE_URL',
                env('MEDIA_GENERATION_DRAFTING_BASE_URL'),
            ),
            'path' => env('MEDIA_GENERATION_DRAFTING_PATH', '/v1/draft'),
            'provider' => env('MEDIA_GENERATION_DRAFTING_PROVIDER', 'llm-adapter'),
            'model' => env('MEDIA_GENERATION_DRAFTING_MODEL', env('MEDIA_GENERATION_DELIVERY_MODEL', 'adapter-managed')),
            'timeout_seconds' => (float) env('MEDIA_GENERATION_DRAFTING_TIMEOUT_SECONDS', env('MEDIA_GENERATION_DELIVERY_TIMEOUT_SECONDS', 30)),
            'connect_timeout_seconds' => (float) env('MEDIA_GENERATION_DRAFTING_CONNECT_TIMEOUT_SECONDS', env('MEDIA_GENERATION_DELIVERY_CONNECT_TIMEOUT_SECONDS', 10)),
            'retry_attempts' => (int) env('MEDIA_GENERATION_DRAFTING_RETRY_ATTEMPTS', env('MEDIA_GENERATION_DELIVERY_RETRY_ATTEMPTS', 2)),
            'retry_sleep_milliseconds' => (int) env('MEDIA_GENERATION_DRAFTING_RETRY_SLEEP_MILLISECONDS', env('MEDIA_GENERATION_DELIVERY_RETRY_SLEEP_MILLISECONDS', 250)),
        ],
        'python' => [
            'base_url' => env('MEDIA_GENERATION_PYTHON_BASE_URL'),
            'generate_path' => env('MEDIA_GENERATION_PYTHON_GENERATE_PATH', '/v1/generate'),
            'health_path' => env('MEDIA_GENERATION_PYTHON_HEALTH_PATH', '/v1/health'),
            'provider' => env('MEDIA_GENERATION_PYTHON_PROVIDER', 'klass-python'),
            'model' => env('MEDIA_GENERATION_PYTHON_MODEL', 'renderer-v1'),
            'shared_secret' => env('MEDIA_GENERATION_PYTHON_SHARED_SECRET'),
            'timeout_seconds' => (float) env('MEDIA_GENERATION_PYTHON_TIMEOUT_SECONDS', 60),
            'connect_timeout_seconds' => (float) env('MEDIA_GENERATION_PYTHON_CONNECT_TIMEOUT_SECONDS', 10),
            'retry_attempts' => (int) env('MEDIA_GENERATION_PYTHON_RETRY_ATTEMPTS', 2),
            'retry_sleep_milliseconds' => (int) env('MEDIA_GENERATION_PYTHON_RETRY_SLEEP_MILLISECONDS', 500),
        ],
        'delivery' => [
            'base_url' => env(
                'MEDIA_GENERATION_LLM_ADAPTER_BASE_URL',
                env('MEDIA_GENERATION_DELIVERY_BASE_URL'),
            ),
            'path' => env('MEDIA_GENERATION_DELIVERY_PATH', '/v1/respond'),
            'provider' => env('MEDIA_GENERATION_DELIVERY_PROVIDER', 'llm-adapter'),
            'model' => env('MEDIA_GENERATION_DELIVERY_MODEL', 'adapter-managed'),
            'timeout_seconds' => (float) env('MEDIA_GENERATION_DELIVERY_TIMEOUT_SECONDS', 30),
            'connect_timeout_seconds' => (float) env('MEDIA_GENERATION_DELIVERY_CONNECT_TIMEOUT_SECONDS', 10),
            'retry_attempts' => (int) env('MEDIA_GENERATION_DELIVERY_RETRY_ATTEMPTS', 2),
            'retry_sleep_milliseconds' => (int) env('MEDIA_GENERATION_DELIVERY_RETRY_SLEEP_MILLISECONDS', 250),
        ],
        'queue' => [
            'connection' => env('MEDIA_GENERATION_QUEUE_CONNECTION', 'database'),
            'name' => env('MEDIA_GENERATION_QUEUE_NAME', 'media-generation'),
            'tries' => (int) env('MEDIA_GENERATION_QUEUE_TRIES', 3),
            'timeout_seconds' => (int) env('MEDIA_GENERATION_QUEUE_TIMEOUT_SECONDS', 300),
            'backoff_seconds' => (int) env('MEDIA_GENERATION_QUEUE_BACKOFF_SECONDS', 30),
            'sleep_seconds' => (int) env('MEDIA_GENERATION_QUEUE_SLEEP_SECONDS', 3),
            'max_jobs' => (int) env('MEDIA_GENERATION_QUEUE_MAX_JOBS', 250),
            'max_time_seconds' => (int) env('MEDIA_GENERATION_QUEUE_MAX_TIME_SECONDS', 3600),
            'memory_mb' => (int) env('MEDIA_GENERATION_QUEUE_MEMORY_MB', 256),
            'concurrency' => max(1, (int) env('MEDIA_GENERATION_QUEUE_CONCURRENCY', 1)),
            'stopwait_seconds' => (int) env('MEDIA_GENERATION_QUEUE_STOPWAIT_SECONDS', 360),
        ],
    ],

];
