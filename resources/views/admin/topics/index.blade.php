@extends('admin.layouts.app')

@section('title', 'Manajemen Topik')
@section('page-title', 'Topics')
@section('page-description', 'Kelola struktur hierarki topik di aplikasi.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">

        {{-- Filter & Search --}}
        <div class="bg-surface border-2 border-black p-6 shadow-drag">
            <form method="GET" action="{{ route('admin.topics.index') }}" class="flex flex-col md:flex-row gap-4">
                <div class="flex-1 relative">
                    <div class="absolute inset-y-0 left-0 flex items-center pl-4 pointer-events-none text-text-muted">
                        <span class="material-symbols-outlined" style="font-size: 20px;">search</span>
                    </div>
                    <input type="text" name="search" value="{{ $search }}" placeholder="Cari judul topik..." class="bg-white border-2 border-black text-text-main text-[14px] font-medium focus:ring-primary focus:border-primary block w-full pl-12 p-3">
                </div>
                <div class="flex gap-3">
                    <button type="submit" class="bg-black hover:bg-gray-800 text-white font-bold px-8 py-3 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase tracking-wider text-[13px]">
                        CARI
                    </button>
                    @if(request()->filled('search'))
                        <a href="{{ route('admin.topics.index') }}" class="flex items-center justify-center bg-white border-2 border-black px-6 py-3 font-bold hover:bg-gray-50 transition uppercase tracking-wider text-[13px]">
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
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider w-16">No</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Judul Topik</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-center">Order</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-center">Status</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-right">Aksi</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-black/10 text-text-main">
                        @forelse($topics as $index => $topic)
                        <tr class="hover:bg-gray-50 transition-colors group">
                            <td class="px-6 py-5 mono-text text-text-muted">
                                {{ $topics->firstItem() + $index }}
                            </td>
                            <td class="px-6 py-5">
                                <div class="font-bold text-[14px]">{{ $topic->title }}</div>
                                <div class="text-[11px] mono-text text-text-muted mt-0.5 uppercase tracking-wider">DIBUAT {{ $topic->created_at->format('d M Y') }}</div>
                            </td>
                            <td class="px-6 py-5">
                                <div class="flex items-center justify-center gap-2">
                                    <form action="{{ route('admin.topics.reorder', $topic->id) }}" method="POST">
                                        @csrf @method('PATCH')
                                        <input type="hidden" name="direction" value="up">
                                        <button type="submit" class="p-1 border border-black hover:bg-primary transition shadow-[1px_1px_0px_rgba(0,0,0,1)] active:shadow-none active:translate-x-[1px] active:translate-y-[1px]" {{ $loop->first && $topics->onFirstPage() ? 'disabled style=opacity:0.3' : '' }}>
                                            <span class="material-symbols-outlined" style="font-size: 18px;">expand_less</span>
                                        </button>
                                    </form>
                                    <span class="mono-text font-bold text-[14px] w-6 text-center">{{ $topic->order }}</span>
                                    <form action="{{ route('admin.topics.reorder', $topic->id) }}" method="POST">
                                        @csrf @method('PATCH')
                                        <input type="hidden" name="direction" value="down">
                                        <button type="submit" class="p-1 border border-black hover:bg-primary transition shadow-[1px_1px_0px_rgba(0,0,0,1)] active:shadow-none active:translate-x-[1px] active:translate-y-[1px]" {{ $loop->last && $topics->onLastPage() ? 'disabled style=opacity:0.3' : '' }}>
                                            <span class="material-symbols-outlined" style="font-size: 18px;">expand_more</span>
                                        </button>
                                    </form>
                                </div>
                            </td>
                            <td class="px-6 py-5 text-center">
                                <form action="{{ route('admin.topics.toggle-publish', $topic->id) }}" method="POST">
                                    @csrf @method('PATCH')
                                    <button type="submit" class="inline-flex items-center px-2 py-0.5 border border-black text-[10px] font-bold uppercase tracking-wider shadow-[2px_2px_0px_rgba(0,0,0,1)] {{ $topic->is_published ? 'bg-primary text-black' : 'bg-gray-100 text-text-muted' }}">
                                        {{ $topic->is_published ? 'Published' : 'Draft' }}
                                    </button>
                                </form>
                            </td>
                            <td class="px-6 py-5 text-right whitespace-nowrap">
                                <a href="{{ route('admin.topics.edit', $topic->id) }}" class="inline-flex items-center gap-1 font-bold text-black border-b-2 border-primary hover:bg-primary/10 transition-colors px-1">
                                    EDIT
                                    <span class="material-symbols-outlined" style="font-size: 16px;">edit</span>
                                </a>
                            </td>
                        </tr>
                        @empty
                        <tr>
                            <td colspan="5" class="py-12 bg-white">
                                @include('admin.partials.empty-state', [
                                    'title'   => 'Topik tidak ditemukan',
                                    'message' => 'Belum ada topik yang dibuat atau sesuai dengan pencarian.',
                                    'icon'    => 'topic_off'
                                ])
                            </td>
                        </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>
            
            {{-- Pagination --}}
            @if($topics->hasPages())
            <div class="px-6 py-6 border-t-2 border-black bg-gray-50">
                {{ $topics->links() }}
            </div>
            @endif
        </div>
    </div>
</div>
@endsection

