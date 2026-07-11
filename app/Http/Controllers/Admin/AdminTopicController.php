<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use App\Models\Topic;
use Illuminate\Http\Request;
use Illuminate\View\View;

class AdminTopicController extends Controller
{
    public function index(Request $request): View
    {
        $search = $request->query('search');
        
        $topics = Topic::query()
            ->when($search, function ($query, $search) {
                $query->where('title', 'like', "%{$search}%");
            })
            ->orderBy('order', 'asc')
            ->paginate(15)
            ->withQueryString();

        return view('admin.topics.index', compact('topics', 'search'));
    }

    public function edit(Topic $topic): View
    {
        return view('admin.topics.edit', compact('topic'));
    }

    public function update(Request $request, Topic $topic)
    {
        $validated = $request->validate([
            'title'        => 'required|string|max:255',
            'is_published' => 'boolean',
        ]);

        $topic->update([
            'title' => $validated['title'],
            'is_published' => $request->has('is_published') ? true : false,
        ]);

        ActivityLog::create([
            'actor_id'     => auth()->id(),
            'action'       => 'update_topic',
            'subject_type' => Topic::class,
            'subject_id'   => $topic->id,
            'metadata'     => $validated,
        ]);

        return redirect()->route('admin.topics.index')->with('success', 'Topic berhasil diperbarui.');
    }

    public function togglePublish(Request $request, Topic $topic)
    {
        $topic->update(['is_published' => !$topic->is_published]);

        ActivityLog::create([
            'actor_id'     => auth()->id(),
            'action'       => 'toggle_topic_publish',
            'subject_type' => Topic::class,
            'subject_id'   => $topic->id,
            'metadata'     => ['new_status' => $topic->is_published],
        ]);

        return back()->with('success', 'Status visibilitas topic diperbarui.');
    }

    public function reorder(Request $request, Topic $topic)
    {
        $direction = $request->input('direction'); // 'up' or 'down'
        
        $currentOrder = $topic->order;
        
        if ($direction === 'up') {
            $swap = Topic::where('order', '<', $currentOrder)->orderBy('order', 'desc')->first();
        } else {
            $swap = Topic::where('order', '>', $currentOrder)->orderBy('order', 'asc')->first();
        }

        if ($swap) {
            $topic->update(['order' => $swap->order]);
            $swap->update(['order' => $currentOrder]);

            ActivityLog::create([
                'actor_id'     => auth()->id(),
                'action'       => 'reorder_topic',
                'subject_type' => Topic::class,
                'subject_id'   => $topic->id,
                'metadata'     => [
                    'direction'  => $direction,
                    'old_order'  => $currentOrder,
                    'new_order'  => $topic->order,
                    'swap_id'    => $swap->id,
                ],
            ]);
        }

        return back()->with('success', 'Urutan topic diperbarui.');
    }
}
