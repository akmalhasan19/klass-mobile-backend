<?php

namespace App\Providers;

use App\Models\User;
use App\Services\MediaContentDraftingService;
use App\Services\MediaGenerationDecisionService;
use Illuminate\Support\Facades\Gate;
use Illuminate\Support\Facades\URL;
use Illuminate\Support\ServiceProvider;

class AppServiceProvider extends ServiceProvider
{
    /**
     * Register any application services.
     */
    public function register(): void
    {
        $this->app->bind(MediaGenerationDecisionService::class, function ($app): MediaGenerationDecisionService {
            return new MediaGenerationDecisionService(
                $app->make(MediaContentDraftingService::class),
            );
        });
    }

    /**
     * Bootstrap any application services.
     */
    public function boot(): void
    {
        if ($this->app->isProduction()) {
            URL::forceScheme('https');
        }

        Gate::define('access-admin-panel', fn (User $user) => $user->isAdmin());
    }
}
