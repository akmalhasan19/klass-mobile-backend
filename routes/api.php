<?php

use App\Http\Controllers\Api\V1\AdminMediaGenerationDebugController;
use App\Http\Controllers\Api\V1\AuthController;
use App\Http\Controllers\Api\V1\AvatarController;
use App\Http\Controllers\Api\V1\ContentController;
use App\Http\Controllers\Api\V1\FileUploadController;
use App\Http\Controllers\Api\V1\FreelancerHiringController;
use App\Http\Controllers\Api\V1\FreelancerSuggestionController;
use App\Http\Controllers\Api\V1\GalleryController;
use App\Http\Controllers\Api\V1\HomepageRecommendationController;
use App\Http\Controllers\Api\V1\HomepageSectionController;
use App\Http\Controllers\Api\V1\MarketplaceTaskController;
use App\Http\Controllers\Api\V1\MediaGenerationController;
use App\Http\Controllers\Api\V1\StudentProgressController;
use App\Http\Controllers\Api\V1\TopicController;
use Illuminate\Support\Facades\Route;

Route::get('/', function () {
    return response()->json([
        'success' => true,
        'message' => 'Klass Mobile API is up and running!',
        'version' => '1.0.0'
    ]);
});

/*
|--------------------------------------------------------------------------
| Klass API Routes (Version 1)
|--------------------------------------------------------------------------
| Semua route di-prefix dengan /api/v1 secara otomatis oleh Laravel.
|
| Struktur:
|   - Public routes (auth)
|   - Protected routes (require Sanctum token)
|   - File upload routes
*/

Route::prefix('v1')->group(function () {

    // =========================================================================
    // Auth Routes (Public)
    // =========================================================================
    Route::prefix('auth')->group(function () {
        Route::post('/register', [AuthController::class, 'register'])
            ->middleware('throttle:3,1');
        Route::post('/login', [AuthController::class, 'login'])
            ->middleware('throttle:5,1');
        Route::post('/get-security-question', [AuthController::class, 'getSecurityQuestion']);
        Route::post('/verify-and-reset-password', [AuthController::class, 'verifyAndResetPassword'])
            ->middleware('throttle:3,1');

        // Protected auth routes
        Route::middleware('auth:sanctum')->group(function () {
            Route::post('/logout', [AuthController::class, 'logout']);
            Route::get('/me', [AuthController::class, 'me']);
            Route::post('/refresh', [AuthController::class, 'refresh']);
        });
    });

    // =========================================================================
    // Public Read-Only API Resources
    // =========================================================================
    Route::get('/topics', [TopicController::class, 'index']);
    Route::get('/topics/{topic}', [TopicController::class, 'show']);
    Route::get('/contents', [ContentController::class, 'index']);
    Route::get('/contents/{content}', [ContentController::class, 'show']);
    Route::get('/marketplace-tasks', [MarketplaceTaskController::class, 'index']);
    Route::get('/marketplace-tasks/{marketplaceTask}', [MarketplaceTaskController::class, 'show']);
    Route::get('/student-progress', [StudentProgressController::class, 'index']);
    Route::get('/student-progress/{studentProgress}', [StudentProgressController::class, 'show']);
    Route::get('/homepage-recommendations', [HomepageRecommendationController::class, 'index']);

    // =========================================================================
    // App Config API (Public)
    // =========================================================================
    Route::get('/homepage-sections', [HomepageSectionController::class, 'index']);

    // =========================================================================
    // Gallery (Public — read-only list of media-rich content)
    // =========================================================================
    Route::get('/gallery', [GalleryController::class, 'index']);

    // =========================================================================
    // Protected Routes (require Sanctum auth)
    // =========================================================================
    Route::middleware('auth:sanctum')->group(function () {
        // Avatar Upload — all authenticated users
        Route::post('/user/avatar', [AvatarController::class, 'store']);

        // Media generation — teacher-only access enforced in controller for strict ownership semantics
        // GET  /media-generations?parent_id={uuid}  → parent-chain history (RF-05)
        // GET  /media-generations?parent_id={uuid}  → 20 most-recent when no parent_id
        Route::get('/media-generations', [MediaGenerationController::class, 'index']);
        Route::post('/media-generations', [MediaGenerationController::class, 'store']);
        Route::get('/media-generations/{mediaGeneration}', [MediaGenerationController::class, 'show']);
        Route::post('/media-generations/{mediaGeneration}/regenerate', [MediaGenerationController::class, 'regenerate']);
        Route::post('/media-generations/{mediaGeneration}/suggest-freelancers', [FreelancerSuggestionController::class, 'suggest']);
        Route::post('/media-generations/{mediaGeneration}/hire-freelancer', [FreelancerHiringController::class, 'hire']);
    });

    // =========================================================================
    // Teacher-Only Routes (require auth + teacher role)
    // =========================================================================
    Route::middleware(['auth:sanctum', 'teacher'])->group(function () {
        // Project creation flow — teachers create educational content
        Route::post('/topics', [TopicController::class, 'store']);
    });

    // =========================================================================
    // Freelancer-Only Routes (require auth + freelancer role)
    // =========================================================================
    Route::middleware(['auth:sanctum', 'freelancer'])->group(function () {
        // Placeholder: freelancer-specific endpoints will be added here
        // e.g. accepting marketplace tasks, managing portfolio, etc.
    });

    // =========================================================================
    // Admin-Protected Write Routes
    // =========================================================================
    Route::middleware(['auth:sanctum', 'admin'])->group(function () {
        Route::get('/admin/media-generations/{mediaGeneration}/debug-taxonomy', [AdminMediaGenerationDebugController::class, 'show']);

        Route::match(['put', 'patch'], '/topics/{topic}', [TopicController::class, 'update']);
        Route::delete('/topics/{topic}', [TopicController::class, 'destroy']);

        Route::post('/contents', [ContentController::class, 'store']);
        Route::match(['put', 'patch'], '/contents/{content}', [ContentController::class, 'update']);
        Route::delete('/contents/{content}', [ContentController::class, 'destroy']);

        Route::post('/marketplace-tasks', [MarketplaceTaskController::class, 'store']);
        Route::match(['put', 'patch'], '/marketplace-tasks/{marketplaceTask}', [MarketplaceTaskController::class, 'update']);
        Route::delete('/marketplace-tasks/{marketplaceTask}', [MarketplaceTaskController::class, 'destroy']);

        Route::post('/student-progress', [StudentProgressController::class, 'store']);
        Route::match(['put', 'patch'], '/student-progress/{studentProgress}', [StudentProgressController::class, 'update']);
        Route::delete('/student-progress/{studentProgress}', [StudentProgressController::class, 'destroy']);

        Route::post('/upload/{category}', [FileUploadController::class, 'upload'])
            ->where('category', 'avatars|gallery|materials|attachments');

        Route::delete('/upload/{category}', [FileUploadController::class, 'destroy'])
            ->where('category', 'avatars|gallery|materials|attachments');
    });

});

