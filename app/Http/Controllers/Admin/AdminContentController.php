<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use App\Models\Content;
use App\Models\Topic;
use Illuminate\Http\Request;
use Illuminate\View\View;

class AdminContentController extends Controller
{
    public function index(Request $request): View
    {
        $search = $request->query('search');
        $topicId = $request->query('topic_id');

        $contents = Content::query()
            ->with('topic')
            ->when($topicId, fn($q) => $q->where('topic_id', $topicId))
            ->when($search, fn($q) => $q->where('title', 'like', "%{$search}%"))
            ->orderBy('order', 'asc')
            ->paginate(15)
            ->withQueryString();

        $topics = Topic::orderBy('order', 'asc')->get();

        return view('admin.contents.index', compact('contents', 'search', 'topicId', 'topics'));
    }

    public function edit(Content $content): View
    {
        $topics = Topic::orderBy('order')->get();
        return view('admin.contents.edit', compact('content', 'topics'));
    }

    public function update(Request $request, Content $content)
    {
        $validated = $request->validate([
            'title'        => 'required|string|max:255',
            'topic_id'     => 'required|exists:topics,id',
            'is_published' => 'boolean',
        ]);

        $content->update([
            'title' => $validated['title'],
            'topic_id' => $validated['topic_id'],
            'is_published' => $request->has('is_published') ? true : false,
        ]);

        ActivityLog::create([
            'actor_id'     => auth()->id(),
            'action'       => 'update_content',
            'subject_type' => Content::class,
            'subject_id'   => $content->id,
            'metadata'     => $validated,
        ]);

        return redirect()->route('admin.contents.index')->with('success', 'Konten berhasil diperbarui.');
    }

    public function togglePublish(Request $request, Content $content)
    {
        $content->update(['is_published' => !$content->is_published]);

        ActivityLog::create([
            'actor_id'     => auth()->id(),
            'action'       => 'toggle_content_publish',
            'subject_type' => Content::class,
            'subject_id'   => $content->id,
            'metadata'     => ['new_status' => $content->is_published],
        ]);

        return back()->with('success', 'Status visibilitas konten diperbarui.');
    }

    public function reorder(Request $request, Content $content)
    {
        $direction = $request->input('direction');
        
        $currentOrder = $content->order;
        
        if ($direction === 'up') {
            $swap = Content::where('topic_id', $content->topic_id)
                ->where('order', '<', $currentOrder)
                ->orderBy('order', 'desc')->first();
        } else {
            $swap = Content::where('topic_id', $content->topic_id)
                ->where('order', '>', $currentOrder)
                ->orderBy('order', 'asc')->first();
        }

        if ($swap) {
            $content->update(['order' => $swap->order]);
            $swap->update(['order' => $currentOrder]);

            ActivityLog::create([
                'actor_id'     => auth()->id(),
                'action'       => 'reorder_content',
                'subject_type' => Content::class,
                'subject_id'   => $content->id,
                'metadata'     => [
                    'direction'  => $direction,
                    'old_order'  => $currentOrder,
                    'new_order'  => $content->order,
                    'swap_id'    => $swap->id,
                ],
            ]);
        }

        return back()->with('success', 'Urutan konten diperbarui.');
    }
}
