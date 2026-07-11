FROM php:8.3-fpm-alpine AS base

WORKDIR /var/www/html

RUN set -eux; \
    apk add --no-cache \
        nginx \
        supervisor \
        su-exec \
        libpq \
        postgresql-dev \
        imagemagick \
        imagemagick-dev \
        ghostscript \
        libzip-dev \
        $PHPIZE_DEPS; \
    docker-php-ext-install -j"$(nproc)" pdo_pgsql opcache zip; \
    pecl install imagick && docker-php-ext-enable imagick; \
    apk del --no-network postgresql-dev imagemagick-dev $PHPIZE_DEPS; \
    rm -rf /var/cache/apk/*; \
    # Allow Imagick to process PDF files (Ghostscript policy)
    if [ -f /etc/ImageMagick-7/policy.xml ]; then \
        sed -i 's/<policy domain="coder" rights="none" pattern="PDF" \/>/<!-- <policy domain="coder" rights="none" pattern="PDF" \/> -->/' /etc/ImageMagick-7/policy.xml; \
    fi; \
    if [ -f /etc/ImageMagick-6/policy.xml ]; then \
        sed -i 's/<policy domain="coder" rights="none" pattern="PDF" \/>/<!-- <policy domain="coder" rights="none" pattern="PDF" \/> -->/' /etc/ImageMagick-6/policy.xml; \
    fi; \
    mv "$PHP_INI_DIR/php.ini-production" "$PHP_INI_DIR/php.ini"; \
    { \
        echo '[opcache]'; \
        echo 'opcache.enable=1'; \
        echo 'opcache.enable_cli=1'; \
        echo 'opcache.validate_timestamps=0'; \
        echo 'opcache.memory_consumption=192'; \
        echo 'opcache.interned_strings_buffer=16'; \
        echo 'opcache.max_accelerated_files=20000'; \
    } > "$PHP_INI_DIR/conf.d/zz-opcache.ini"; \
    { \
        echo '[global]'; \
        echo 'error_log = /proc/self/fd/2'; \
        echo 'daemonize = no'; \
        echo; \
        echo '[www]'; \
        echo 'user = www-data'; \
        echo 'group = www-data'; \
        echo 'listen = 127.0.0.1:9000'; \
        echo 'listen.owner = www-data'; \
        echo 'listen.group = www-data'; \
        echo 'pm = dynamic'; \
        echo 'pm.max_children = 10'; \
        echo 'pm.start_servers = 2'; \
        echo 'pm.min_spare_servers = 1'; \
        echo 'pm.max_spare_servers = 3'; \
        echo 'clear_env = no'; \
        echo 'catch_workers_output = yes'; \
        echo 'decorate_workers_output = no'; \
        echo 'access.log = /proc/self/fd/2'; \
        echo 'php_admin_flag[log_errors] = on'; \
        echo 'php_admin_value[error_log] = /proc/self/fd/2'; \
    } > /usr/local/etc/php-fpm.d/zz-render.conf; \
    rm -f /usr/local/etc/php-fpm.d/www.conf; \
    mkdir -p /home/www-data/.postgresql /run/nginx /var/lib/nginx/tmp /var/log/supervisor /var/www/html/storage /var/www/html/bootstrap/cache; \
    chmod 700 /home/www-data /home/www-data/.postgresql; \
    chown -R www-data:www-data /home/www-data /var/www/html /run/nginx /var/lib/nginx

FROM composer:2.8 AS vendor

WORKDIR /var/www/html

COPY composer.json composer.lock ./

RUN composer install \
    --no-dev \
    --no-interaction \
    --prefer-dist \
    --optimize-autoloader \
    --no-scripts

FROM node:22-alpine AS frontend

WORKDIR /var/www/html

COPY package.json ./

RUN npm install --no-fund --no-audit

COPY resources ./resources
COPY vite.config.js ./vite.config.js

RUN npm run build

FROM base AS runtime

ENV APP_ENV=production \
    APP_DEBUG=false \
    LOG_CHANNEL=stderr \
    LOG_STACK=single \
    HOME=/home/www-data \
    DB_QUEUE_RETRY_AFTER=420 \
    MEDIA_GENERATION_QUEUE_CONNECTION=database \
    MEDIA_GENERATION_QUEUE_NAME=media-generation \
    MEDIA_GENERATION_QUEUE_TRIES=3 \
    MEDIA_GENERATION_QUEUE_TIMEOUT_SECONDS=300 \
    MEDIA_GENERATION_QUEUE_BACKOFF_SECONDS=30 \
    MEDIA_GENERATION_QUEUE_SLEEP_SECONDS=3 \
    MEDIA_GENERATION_QUEUE_MAX_JOBS=250 \
    MEDIA_GENERATION_QUEUE_MAX_TIME_SECONDS=3600 \
    MEDIA_GENERATION_QUEUE_MEMORY_MB=256 \
    MEDIA_GENERATION_QUEUE_CONCURRENCY=1 \
    MEDIA_GENERATION_QUEUE_STOPWAIT_SECONDS=360 \
    PORT=7860

COPY . .
COPY --from=vendor /var/www/html/vendor ./vendor
COPY --from=frontend /var/www/html/public/build ./public/build
COPY docker/nginx.conf /etc/nginx/nginx.conf.template
COPY docker/supervisord.conf /etc/supervisor/conf.d/supervisord.conf
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh

RUN set -eux; \
    chmod +x /usr/local/bin/entrypoint.sh; \
    mkdir -p /home/www-data/.postgresql storage/framework/cache storage/framework/sessions storage/framework/views storage/logs bootstrap/cache; \
    rm -f bootstrap/cache/config.php; \
    chmod 700 /home/www-data /home/www-data/.postgresql; \
    chown -R www-data:www-data /home/www-data /var/www/html/storage /var/www/html/bootstrap/cache /run/nginx /var/lib/nginx

# Hugging Face Spaces uses port 7860 by default.
EXPOSE 7860

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
