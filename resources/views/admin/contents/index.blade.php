@extends('admin.layouts.app')

@section('title', 'Manajemen Konten')
@section('page-title', 'Contents')
@section('page-description', 'Kelola materi/konten pembelajaran aplikasi.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">

        {{-- Filter & Search --}}
        <div class="bg-surface border-2 border-black p-6 shadow-drag">
            <form method="GET" action="{{ route('admin.contents.index') }}" class="grid grid-cols-1 md:grid-cols-12 gap-4">
                
                <div class="md:col-span-4 relative font-bold">
                    <select name="topic_id" class="bg-white border-2 border-black text-text-main text-[14px] font-bold focus:ring-primary focus:border-primary block w-full p-3 pr-10 appearance-none">
                        <option value="">Semua Topik</option>
                        @foreach($topics as $topic)
                            <option value="{{ $topic->id }}" {{ $topicId == $topic->id ? 'selected' : '' }}>
                                {{ $topic->title }} (Order: {{ $topic->order }})
                            </option>
                        @endforeach
                    </select>
                    <div class="absolute inset-y-0 right-0 flex items-center pr-3 pointer-events-none">
                        <span class="material-symbols-outlined">expand_more</span>
                    </div>
                </div>

                <div class="md:col-span-5 relative">
                    <div class="absolute inset-y-0 left-0 flex items-center pl-4 pointer-events-none text-text-muted">
                        <span class="material-symbols-outlined" style="font-size: 20px;">search</span>
                    </div>
                    <input type="text" name="search" value="{{ $search }}" placeholder="Cari judul konten..." class="bg-white border-2 border-black text-text-main text-[14px] font-medium focus:ring-primary focus:border-primary block w-full pl-12 p-3">
                </div>

                <div class="md:col-span-3 flex gap-3">
                    <button type="submit" class="flex-1 bg-black hover:bg-gray-800 text-white font-bold py-3 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase tracking-wider text-[13px]">
                        FILTER
                    </button>
                    @if(request()->filled('search') || request()->filled('topic_id'))
                        <a href="{{ route('admin.contents.index') }}" class="flex items-center justify-center bg-white border-2 border-black px-4 py-3 font-bold hover:bg-gray-50 transition uppercase tracking-wider text-[13px]">
                            RESET
                        </a>
                    @endif
                </div>
            </form>
        </div>

        {{-- Info Reorder --}}
        @if($topicId)
        <div class="bg-primary/10 border-2 border-black p-4 shadow-[4px_4px_0px_rgba(0,0,0,1)] flex items-start gap-4">
            <div class="bg-primary border-2 border-black p-1 shrink-0 shadow-[2px_2px_0px_rgba(0,0,0,1)]">
                <span class="material-symbols-outlined block text-black" style="font-size: 20px;">info</span>
            </div>
            <p class="text-[13px] font-bold text-black uppercase tracking-tight pt-1">
                Anda memfilter berdasarkan topik spesifik. Panah reorder di bawah ini akan memindahkan urutan konten di dalam topik <span class="bg-black text-white px-1 ml-1">{{ $topics->firstWhere('id', $topicId)->title ?? 'ini' }}</span>.
            </p>
        </div>
        @endif

        {{-- Table Container --}}
        <div class="bg-surface border-2 border-black shadow-drag overflow-hidden">
            <div class="overflow-x-auto">
                <table class="w-full text-left text-[13px]">
                    <thead class="bg-gray-50 border-b-2 border-black">
                        <tr>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider w-16">No</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Konten & Topik</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-center">Order</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-center">Status</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-right">Aksi</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-black/10 text-text-main">
                        @forelse($contents as $index => $content)
                        <tr class="hover:bg-gray-50 transition-colors group">
                            <td class="px-6 py-5 mono-text text-text-muted">{{ $contents->firstItem() + $index }}</td>
                            <td class="px-6 py-5">
                                <div class="font-bold text-[14px]">{{ $content->title }}</div>
                                <div class="text-[11px] mono-text text-text-muted mt-0.5 uppercase tracking-wider">Topik: {{ $content->topic?->title ?? '-' }}</div>
                            </td>
                            <td class="px-6 py-5">
                                @if($topicId)
                                <div class="flex items-center justify-center gap-2">
                                    <form action="{{ route('admin.contents.reorder', $content->id) }}" method="POST">
                                        @csrf @method('PATCH')
                                        <input type="hidden" name="direction" value="up">
                                        <button type="submit" class="p-1 border border-black hover:bg-primary transition shadow-[1px_1px_0px_rgba(0,0,0,1)] active:shadow-none active:translate-x-[1px] active:translate-y-[1px]" {{ $loop->first && $contents->onFirstPage() ? 'disabled style=opacity:0.3' : '' }}>
                                            <span class="material-symbols-outlined" style="font-size: 18px;">expand_less</span>
                                        </button>
                                    </form>
                                    <span class="mono-text font-bold text-[14px] w-6 text-center">{{ $content->order }}</span>
                                    <form action="{{ route('admin.contents.reorder', $content->id) }}" method="POST">
                                        @csrf @method('PATCH')
                                        <input type="hidden" name="direction" value="down">
                                        <button type="submit" class="p-1 border border-black hover:bg-primary transition shadow-[1px_1px_0px_rgba(0,0,0,1)] active:shadow-none active:translate-x-[1px] active:translate-y-[1px]" {{ $loop->last && $contents->onLastPage() ? 'disabled style=opacity:0.3' : '' }}>
                                            <span class="material-symbols-outlined" style="font-size: 18px;">expand_more</span>
                                        </button>
                                    </form>
                                </div>
                                @else
                                <div class="text-center">
                                    <span class="inline-flex border border-black px-1.5 py-0.5 bg-gray-50 text-[11px] mono-text text-text-muted shadow-[1px_1px_0px_rgba(0,0,0,1)]" title="Pilih spesifik topik terlebih dahulu untuk reorder">
                                        #{{ $content->order }}
                                    </span>
                                </div>
                                @endif
                            </td>
                            <td class="px-6 py-5 text-center">
                                <form action="{{ route('admin.contents.toggle-publish', $content->id) }}" method="POST">
                                    @csrf @method('PATCH')
                                    <button type="submit" class="inline-flex items-center px-2 py-0.5 border border-black text-[10px] font-bold uppercase tracking-wider shadow-[2px_2px_0px_rgba(0,0,0,1)] {{ $content->is_published ? 'bg-primary text-black' : 'bg-gray-100 text-text-muted' }}">
                                        {{ $content->is_published ? 'Published' : 'Draft' }}
                                    </button>
                                </form>
                            </td>
                            <td class="px-6 py-5 text-right whitespace-nowrap">
                                <a href="{{ route('admin.contents.edit', $content->id) }}" class="inline-flex items-center gap-1 font-bold text-black border-b-2 border-primary hover:bg-primary/10 transition-colors px-1">
                                    EDIT
                                    <span class="material-symbols-outlined" style="font-size: 16px;">edit</span>
                                </a>
                            </td>
                        </tr>
                        @empty
                        <tr>
                            <td colspan="5" class="py-12 bg-white text-center">
                                @include('admin.partials.empty-state', [
                                    'title'   => 'Konten tidak ditemukan',
                                    'message' => 'Belum ada konten yang dibuat atau sesuai dengan pencarian/filter.',
                                    'icon'    => 'content_paste_off'
                                ])
                            </td>
                        </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>
            
            {{-- Pagination --}}
            @if($contents->hasPages())
            <div class="px-6 py-6 border-t-2 border-black bg-gray-50">
                {{ $contents->links() }}
            </div>
            @endif
        </div>
    </div>
</div>
@endsection

