@extends('admin.layouts.app')

@section('title', 'Marketplace Tasks')
@section('page-title', 'Tasks Moderation')
@section('page-description', 'Kelola, awasi status, dan moderasi tugas marketplace.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">

        {{-- Filter & Search --}}
        <div class="bg-surface border-2 border-black p-6 shadow-drag">
            <form method="GET" action="{{ route('admin.tasks.index') }}" class="grid grid-cols-1 md:grid-cols-12 gap-4">
                
                <div class="md:col-span-3 relative font-bold">
                    <select name="status" class="bg-white border-2 border-black text-text-main text-[14px] font-bold focus:ring-primary focus:border-primary block w-full p-3 pr-10 appearance-none">
                        <option value="">Semua Status</option>
                        <option value="open" {{ $status == 'open' ? 'selected' : '' }}>Open</option>
                        <option value="taken" {{ $status == 'taken' ? 'selected' : '' }}>Taken (Diambil)</option>
                        <option value="in_progress" {{ $status == 'in_progress' ? 'selected' : '' }}>In Progress</option>
                        <option value="completed" {{ $status == 'completed' ? 'selected' : '' }}>Completed</option>
                        <option value="verified" {{ $status == 'verified' ? 'selected' : '' }}>Verified / Closed</option>
                    </select>
                    <div class="absolute inset-y-0 right-0 flex items-center pr-3 pointer-events-none">
                        <span class="material-symbols-outlined">expand_more</span>
                    </div>
                </div>

                <div class="md:col-span-6 relative">
                    <div class="absolute inset-y-0 left-0 flex items-center pl-4 pointer-events-none text-text-muted">
                        <span class="material-symbols-outlined" style="font-size: 20px;">search</span>
                    </div>
                    <input type="text" name="search" value="{{ $search }}" placeholder="Cari berdasarkan judul konten terkait..." class="bg-white border-2 border-black text-text-main text-[14px] font-medium focus:ring-primary focus:border-primary block w-full pl-12 p-3">
                </div>

                <div class="md:col-span-3 flex gap-3">
                    <button type="submit" class="flex-1 bg-black hover:bg-gray-800 text-white font-bold py-3 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase tracking-wider text-[13px]">
                        FILTER
                    </button>
                    @if(request()->filled('search') || request()->filled('status'))
                        <a href="{{ route('admin.tasks.index') }}" class="flex items-center justify-center bg-white border-2 border-black px-4 py-3 font-bold hover:bg-gray-50 transition uppercase tracking-wider text-[13px]">
                            RESET
                        </a>
                    @endif
                </div>
            </form>
        </div>

        {{-- Table Container --}}
        <div class="bg-surface border-2 border-black shadow-drag overflow-hidden">
            <div class="overflow-x-auto">
                <table class="w-full text-left text-[13px]">
                    <thead class="bg-gray-50 border-b-2 border-black">
                        <tr>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Konten (Task Parent)</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Dibuat Pada</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-center">Status</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-right">Aksi</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-black/10 text-text-main">
                        @forelse($tasks as $task)
                        <tr class="hover:bg-gray-50 transition-colors group">
                            <td class="px-6 py-5">
                                @if($task->content)
                                <div class="flex items-center gap-4">
                                    <div class="w-10 h-10 border border-black bg-white flex items-center justify-center font-bold text-[14px] shadow-[2px_2px_0px_rgba(0,0,0,1)] flex-shrink-0">
                                        <span class="material-symbols-outlined" style="font-size: 20px;">task</span>
                                    </div>
                                    <div class="max-w-sm">
                                        <div class="font-bold text-[14px] truncate" title="{{ $task->content->title }}">{{ $task->content->title }}</div>
                                        <div class="text-[11px] mono-text text-text-muted mt-0.5 uppercase tracking-wider">TOPIK: {{ $task->content->topic?->title ?? 'TANPA TOPIK' }}</div>
                                    </div>
                                </div>
                                @else
                                <div class="flex items-center gap-4 text-red-600 font-bold">
                                    <div class="w-10 h-10 border border-black bg-red-50 flex items-center justify-center shadow-[2px_2px_0px_rgba(0,0,0,1)] flex-shrink-0">
                                        <span class="material-symbols-outlined">error</span>
                                    </div>
                                    KONTEN INDUK HILANG
                                </div>
                                @endif
                            </td>
                            <td class="px-6 py-5 mono-text text-text-muted whitespace-nowrap">
                                {{ $task->created_at->format('d M Y, H:i') }}
                            </td>
                            <td class="px-6 py-5 text-center">
                                @php
                                    $statusColor = match($task->status) {
                                        'open' => 'bg-emerald-500 text-white',
                                        'taken', 'in_progress' => 'bg-blue-500 text-white',
                                        'completed', 'verified' => 'bg-gray-100 text-gray-700',
                                        default => 'bg-white text-gray-400',
                                    };
                                @endphp
                                <span class="inline-flex items-center px-2.5 py-1 border-2 border-black text-[10px] font-bold uppercase tracking-wider shadow-[2px_2px_0px_rgba(0,0,0,1)] {{ $statusColor }}">
                                    {{ $task->status ?? 'UNKNOWN' }}
                                </span>
                            </td>
                            <td class="px-6 py-5 text-right whitespace-nowrap">
                                <a href="{{ route('admin.tasks.show', $task->id) }}" class="inline-flex items-center gap-1 font-bold text-black border-b-2 border-primary hover:bg-primary/10 transition-colors px-1">
                                    TINJAU
                                    <span class="material-symbols-outlined" style="font-size: 16px;">gavel</span>
                                </a>
                            </td>
                        </tr>
                        @empty
                        <tr>
                            <td colspan="4" class="py-12 bg-white">
                                @include('admin.partials.empty-state', [
                                    'title'   => 'Task tidak ditemukan',
                                    'message' => 'Belum ada task yang diposting atau kriteria pencarian tidak cocok.',
                                    'icon'    => 'assignment_late'
                                ])
                            </td>
                        </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>
            
            {{-- Pagination --}}
            @if($tasks->hasPages())
            <div class="px-6 py-6 border-t-2 border-black bg-gray-50">
                {{ $tasks->links() }}
            </div>
            @endif
        </div>
    </div>
</div>
@endsection
