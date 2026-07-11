<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use App\Models\Content;
use App\Models\MarketplaceTask;
use App\Models\MediaFile;
use App\Models\Topic;
use App\Models\User;
use Carbon\Carbon;
use Illuminate\Http\Request;
use Illuminate\View\View;

class AdminDashboardController extends Controller
{
    /**
     * Dashboard admin - menampilkan data monitoring
     */
    public function index(Request $request): View
    {
        $period = $request->query('period', 'all');

        $dateFilter = null;
        if ($period === 'today') {
            $dateFilter = Carbon::today();
        } elseif ($period === '7_days') {
            $dateFilter = Carbon::today()->subDays(7);
        } elseif ($period === '30_days') {
            $dateFilter = Carbon::today()->subDays(30);
        }

        $applyDateFilter = function ($query) use ($dateFilter) {
            if ($dateFilter) {
                $query->where('created_at', '>=', $dateFilter);
            }
        };

        // Summary Counts
        $usersCount = User::when($dateFilter, $applyDateFilter)->count();
        $topicsCount = Topic::when($dateFilter, $applyDateFilter)->count();
        $contentsCount = Content::when($dateFilter, $applyDateFilter)->count();
        $tasksCount = MarketplaceTask::when($dateFilter, $applyDateFilter)->count();
        $mediaCount = MediaFile::when($dateFilter, $applyDateFilter)->count();
        $activityCount = ActivityLog::when($dateFilter, $applyDateFilter)->count();

        // Recent Data
        $recentUsers = User::when($dateFilter, $applyDateFilter)->latest()->take(5)->get();
        $recentContents = Content::when($dateFilter, $applyDateFilter)->with('topic')->latest()->take(5)->get();
        $recentTasks = MarketplaceTask::when($dateFilter, $applyDateFilter)->with('content')->latest()->take(5)->get();
        $recentMedia = MediaFile::when($dateFilter, $applyDateFilter)->latest()->take(5)->get();
        $recentActivity = ActivityLog::when($dateFilter, $applyDateFilter)->with('actor')->latest()->take(10)->get();

        return view('admin.dashboard', compact(
            'period',
            'usersCount',
            'topicsCount',
            'contentsCount',
            'tasksCount',
            'mediaCount',
            'activityCount',
            'recentUsers',
            'recentContents',
            'recentTasks',
            'recentMedia',
            'recentActivity'
        ));
    }
}
