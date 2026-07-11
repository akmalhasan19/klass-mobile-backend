<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use Illuminate\Http\Request;
use Illuminate\View\View;

class AdminActivityLogController extends Controller
{
    /**
     * Tampilkan daftar activity log.
     */
    public function index(Request $request): View
    {
        $action = $request->query('action');
        $actorId = $request->query('actor_id');
        $entityType = $request->query('subject_type');
        $dateFrom = $request->query('date_from');
        $dateTo = $request->query('date_to');
        $search = $request->query('search');

        $logs = ActivityLog::query()
            ->with(['actor'])
            ->when($action, fn($q) => $q->where('action', $action))
            ->when($actorId, fn($q) => $q->where('actor_id', $actorId))
            ->when($entityType, fn($q) => $q->where('subject_type', $entityType))
            ->when($dateFrom, fn($q) => $q->whereDate('created_at', '>=', $dateFrom))
            ->when($dateTo, fn($q) => $q->whereDate('created_at', '<=', $dateTo))
            ->when($search, function($q) use ($search) {
                return $q->where(function($qq) use ($search) {
                    $qq->whereHas('actor', function($sq) use ($search) {
                        $sq->where('name', 'like', "%{$search}%")
                           ->orWhere('email', 'like', "%{$search}%");
                    })->orWhere('subject_id', 'like', "%{$search}%")
                      ->orWhere('action', 'like', "%{$search}%");
                });
            })
            ->latest()
            ->paginate(20)
            ->withQueryString();

        // Data for filters
        $actions = ActivityLog::select('action')->distinct()->pluck('action');
        $entityTypes = ActivityLog::select('subject_type')->distinct()->pluck('subject_type');
        $actors = ActivityLog::with('actor')->select('actor_id')->distinct()->get()->map(function($log) {
            return $log->actor;
        })->filter();

        return view('admin.activity-logs.index', compact(
            'logs', 'action', 'actorId', 'entityType', 'dateFrom', 'dateTo', 'actions', 'entityTypes', 'actors', 'search'
        ));
    }
}
