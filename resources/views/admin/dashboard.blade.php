@extends('admin.layouts.app')

@section('title', 'Dashboard')

@section('page-title', 'Dashboard')
@section('page-description', 'Ringkasan operasional aplikasi Klass')

@section('topbar-actions')
    <form method="GET" action="{{ route('admin.dashboard') }}" class="flex items-center gap-2">
        <label for="period" class="text-[11px] uppercase tracking-wider text-text-muted mono-text font-medium hidden sm:block">Filter:</label>
        <select name="period" id="period" onchange="this.form.submit()" class="bg-surface border-border text-text-main text-[12px] rounded-none focus:ring-black focus:border-black block py-1.5 px-3">
            <option value="all" {{ $period === 'all' ? 'selected' : '' }}>Semua Waktu</option>
            <option value="today" {{ $period === 'today' ? 'selected' : '' }}>Hari Ini</option>
            <option value="7_days" {{ $period === '7_days' ? 'selected' : '' }}>7 Hari Terakhir</option>
            <option value="30_days" {{ $period === '30_days' ? 'selected' : '' }}>30 Hari Terakhir</option>
        </select>
    </form>
@endsection

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">
        
        {{-- Welcome banner --}}
        <div class="relative overflow-hidden bg-primary border-2 border-black p-8 shadow-drag">
            <div class="relative z-10">
                <h2 class="text-2xl font-bold text-black tracking-tight font-display">Selamat datang, {{ Auth::user()->name }}! 👋</h2>
                <p class="text-[14px] text-black/80 mt-2 font-medium">
                    Panel admin Klass siap digunakan. Berikut adalah ringkasan data monitoring operasional sistem.
                </p>
            </div>
        </div>

        {{-- Summary cards --}}
        <div class="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-6">
            @foreach([
                ['label' => 'Total Users',      'value' => number_format($usersCount)],
                ['label' => 'Total Topics',     'value' => number_format($topicsCount)],
                ['label' => 'Total Contents',   'value' => number_format($contentsCount)],
                ['label' => 'Marketplace Tasks','value' => number_format($tasksCount)],
                ['label' => 'Media Files',      'value' => number_format($mediaCount)],
                ['label' => 'Activity Logs',    'value' => number_format($activityCount)],
            ] as $card)
            <div class="bg-surface border border-border p-5 hover:border-black hover:shadow-drag transition-all group flex flex-col justify-between">
                <p class="text-[11px] mono-text text-text-muted uppercase tracking-wider mt-0.5 mb-2">{{ $card['label'] }}</p>
                <p class="text-2xl font-bold text-text-main tracking-tight">{{ $card['value'] }}</p>
            </div>
            @endforeach
        </div>

        {{-- Recent Items Grid --}}
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
            
            {{-- Recent Users --}}
            <div class="flex flex-col bg-surface border border-border shadow-sm h-full">
                <div class="p-4 border-b border-border bg-gray-50 flex items-center justify-between">
                    <h3 class="text-[13px] font-semibold uppercase tracking-wide text-text-main flex items-center gap-2">
                        <span class="material-symbols-outlined" style="font-size: 16px;">group_add</span>
                        User Baru
                    </h3>
                    <span class="text-[11px] mono-text text-text-muted">{{ count($recentUsers) }} ITEMS</span>
                </div>
                <div class="flex-1 overflow-y-auto p-4 space-y-3">
                    @if($recentUsers->isEmpty())
                        <div class="text-[13px] text-text-muted font-medium text-center py-4">Tidak ada user baru di periode ini.</div>
                    @else
                        @foreach($recentUsers as $user)
                        <div class="group flex items-center bg-surface border border-border p-3 hover:border-gray-400 transition-colors">
                            <div class="flex-1">
                                <p class="text-[14px] font-semibold text-text-main">{{ $user->name }}</p>
                                <p class="text-[11px] mono-text text-text-muted mt-0.5">{{ $user->email }}</p>
                            </div>
                            <div class="text-[11px] mono-text text-text-muted whitespace-nowrap">
                                {{ $user->created_at->diffForHumans() }}
                            </div>
                        </div>
                        @endforeach
                    @endif
                </div>
            </div>

            {{-- Recent Contents --}}
            <div class="flex flex-col bg-surface border border-border shadow-sm h-full">
                <div class="p-4 border-b border-border bg-gray-50 flex items-center justify-between">
                    <h3 class="text-[13px] font-semibold uppercase tracking-wide text-text-main flex items-center gap-2">
                        <span class="material-symbols-outlined" style="font-size: 16px;">post_add</span>
                        Konten Baru
                    </h3>
                    <span class="text-[11px] mono-text text-text-muted">{{ count($recentContents) }} ITEMS</span>
                </div>
                <div class="flex-1 overflow-y-auto p-4 space-y-3">
                    @if($recentContents->isEmpty())
                        <div class="text-[13px] text-text-muted font-medium text-center py-4">Tidak ada konten baru di periode ini.</div>
                    @else
                        @foreach($recentContents as $content)
                        <div class="group flex items-center bg-surface border border-border p-3 hover:border-gray-400 transition-colors">
                            <div class="flex-1 truncate mr-3">
                                <p class="text-[14px] font-semibold text-text-main truncate">{{ $content->title }}</p>
                                <p class="text-[11px] mono-text text-text-muted mt-0.5 truncate">TOPIK: {{ strtoupper($content->topic?->title ?? 'TANPA TOPIK') }}</p>
                            </div>
                            <div class="text-[11px] mono-text text-text-muted whitespace-nowrap">
                                {{ $content->created_at->diffForHumans() }}
                            </div>
                        </div>
                        @endforeach
                    @endif
                </div>
            </div>

        </div>
    </div>
</div>
@endsection

