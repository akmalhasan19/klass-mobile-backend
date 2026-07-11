<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use App\Models\MarketplaceTask;
use Illuminate\Http\Request;
use Illuminate\View\View;

class AdminTaskController extends Controller
{
    public function index(Request $request): View
    {
        $status = $request->query('status');
        $search = $request->query('search');

        $tasks = MarketplaceTask::query()
            ->with(['content.topic'])
            ->when($status, fn($q) => $q->where('status', $status))
            ->when($search, function ($query, $search) {
                $query->whereHas('content', function ($q) use ($search) {
                    $q->where('title', 'like', "%{$search}%");
                });
            })
            ->latest()
            ->paginate(15)
            ->withQueryString();

        return view('admin.tasks.index', compact('tasks', 'status', 'search'));
    }

    public function show(MarketplaceTask $task): View
    {
        $task->load(['content.topic']);
        return view('admin.tasks.show', compact('task'));
    }

    public function updateStatus(Request $request, MarketplaceTask $task)
    {
        $request->validate([
            'status' => 'required|string|max:50',
        ]);

        $oldStatus = $task->status;
        $newStatus = $request->status;

        if ($oldStatus !== $newStatus) {
            $task->update(['status' => $newStatus]);

            ActivityLog::create([
                'actor_id'     => auth()->id(),
                'action'       => 'update_task_status',
                'subject_type' => MarketplaceTask::class,
                'subject_id'   => $task->id,
                'metadata'     => [
                    'old_status' => $oldStatus,
                    'new_status' => $newStatus,
                ],
            ]);
        }

        return back()->with('success', 'Status task berhasil diperbarui.');
    }

    public function destroy(MarketplaceTask $task)
    {
        $taskId = $task->id;
        
        $task->delete();

        ActivityLog::create([
            'actor_id'     => auth()->id(),
            'action'       => 'delete_task',
            'subject_type' => MarketplaceTask::class,
            'subject_id'   => $taskId,
            'metadata'     => ['reason' => 'Admin moderation'],
        ]);

        return redirect()->route('admin.tasks.index')->with('success', 'Task berhasil dihapus (Moderasi).');
    }
}
