<?php

use App\Http\Controllers\Admin\AdminAuthController;
use App\Http\Controllers\Admin\AdminDashboardController;
use App\Http\Controllers\Admin\AdminMediaGenerationController;
use App\Http\Controllers\Admin\AdminUserController;
use App\Http\Controllers\Admin\AdminTopicController;
use App\Http\Controllers\Admin\AdminContentController;
use App\Http\Controllers\Admin\AdminTaskController;
use App\Http\Controllers\Admin\AdminMediaController;
use App\Http\Controllers\Admin\AdminActivityLogController;
use App\Http\Controllers\Admin\AdminHomepageSectionController;
use App\Http\Controllers\Admin\AdminRecommendedProjectController;
use App\Http\Controllers\Admin\AdminSystemSettingController;
use Illuminate\Support\Facades\Route;

/*
|--------------------------------------------------------------------------
| Web Routes
|--------------------------------------------------------------------------
|
| Here is where you can register web routes for your application. These
| routes are loaded by the RouteServiceProvider and all of them will
| be assigned to the "web" middleware group. Make something great!
|
*/

Route::get('/', function () {
    return redirect()->route('admin.login');
});

// --------------------------------------------------------------------------
// Admin — Route publik (login, tidak perlu autentikasi)
// --------------------------------------------------------------------------
Route::prefix('admin')
    ->name('admin.')
    ->group(function () {
        Route::get('/login', [AdminAuthController::class, 'showLogin'])->name('login');
        Route::post('/login', [AdminAuthController::class, 'login'])->name('login.post');
    });

// --------------------------------------------------------------------------
// Admin — Route terproteksi (melalui EnsureUserIsAdmin middleware)
// --------------------------------------------------------------------------
Route::prefix('admin')
    ->name('admin.')
    ->middleware(['admin'])
    ->group(function () {
        
        // Base / Dashboard
        Route::post('/logout', [AdminAuthController::class, 'logout'])->name('logout');
        Route::get('/', [AdminDashboardController::class, 'index'])->name('dashboard');

        // User Management
        Route::get('/users', [AdminUserController::class, 'index'])->name('users.index');
        Route::get('/users/{user}', [AdminUserController::class, 'show'])->name('users.show');
        Route::patch('/users/{user}/role', [AdminUserController::class, 'updateRole'])->name('users.update-role');

        // Topic Management
        Route::get('/topics', [AdminTopicController::class, 'index'])->name('topics.index');
        Route::get('/topics/{topic}/edit', [AdminTopicController::class, 'edit'])->name('topics.edit');
        Route::patch('/topics/{topic}', [AdminTopicController::class, 'update'])->name('topics.update');
        Route::patch('/topics/{topic}/reorder', [AdminTopicController::class, 'reorder'])->name('topics.reorder');
        Route::patch('/topics/{topic}/publish', [AdminTopicController::class, 'togglePublish'])->name('topics.toggle-publish');

        // Content Management
        Route::get('/contents', [AdminContentController::class, 'index'])->name('contents.index');
        Route::get('/contents/{content}/edit', [AdminContentController::class, 'edit'])->name('contents.edit');
        Route::patch('/contents/{content}', [AdminContentController::class, 'update'])->name('contents.update');
        Route::patch('/contents/{content}/reorder', [AdminContentController::class, 'reorder'])->name('contents.reorder');
        Route::patch('/contents/{content}/publish', [AdminContentController::class, 'togglePublish'])->name('contents.toggle-publish');

        // Marketplace Tasks
        Route::get('/tasks', [AdminTaskController::class, 'index'])->name('tasks.index');
        Route::get('/tasks/{task}', [AdminTaskController::class, 'show'])->name('tasks.show');
        Route::patch('/tasks/{task}/status', [AdminTaskController::class, 'updateStatus'])->name('tasks.update-status');
        Route::delete('/tasks/{task}', [AdminTaskController::class, 'destroy'])->name('tasks.destroy');

        // Media Management
        Route::get('/media', [AdminMediaController::class, 'index'])->name('media.index');
        Route::post('/media', [AdminMediaController::class, 'store'])->name('media.store');
        Route::delete('/media/bulk', [AdminMediaController::class, 'bulkDestroy'])->name('media.bulk-destroy');
        Route::delete('/media/{media}', [AdminMediaController::class, 'destroy'])->name('media.destroy');

        // Media Generation Debug
        Route::get('/media-generations', [AdminMediaGenerationController::class, 'index'])->name('media-generations.index');

        // Activity Logs
        Route::get('/activity-logs', [AdminActivityLogController::class, 'index'])->name('activity-logs.index');

        // Homepage Sections
        Route::get('/homepage-sections', [AdminHomepageSectionController::class, 'index'])->name('homepage-sections.index');
        Route::patch('/homepage-sections', [AdminHomepageSectionController::class, 'update'])->name('homepage-sections.update');
        Route::post('/homepage-sections/recommended-projects', [AdminRecommendedProjectController::class, 'store'])->name('recommended-projects.store');
        Route::put('/homepage-sections/recommended-projects/{recommendedProject}', [AdminRecommendedProjectController::class, 'update'])->name('recommended-projects.update');
        Route::delete('/homepage-sections/recommended-projects/{recommendedProject}', [AdminRecommendedProjectController::class, 'destroy'])->name('recommended-projects.destroy');
        Route::patch('/homepage-sections/recommended-projects/{recommendedProject}/toggle-active', [AdminRecommendedProjectController::class, 'toggleActive'])->name('recommended-projects.toggle-active');
        Route::patch('/homepage-sections/recommended-projects/{recommendedProject}/show-now', [AdminRecommendedProjectController::class, 'showNow'])->name('recommended-projects.show-now');

        // System Settings
        Route::get('/settings', [AdminSystemSettingController::class, 'index'])->name('settings.index');
        Route::patch('/settings', [AdminSystemSettingController::class, 'update'])->name('settings.update');

    });
