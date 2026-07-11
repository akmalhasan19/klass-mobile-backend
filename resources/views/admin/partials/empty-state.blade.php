{{--
    Admin Empty State
    Usage: @include('admin.partials.empty-state', [
        'title'   => 'Belum ada data',
        'message' => 'Belum ada pengguna yang terdaftar.',
        'icon'    => 'users',   // optional
    ])
--}}
<div class="flex flex-col items-center justify-center py-16 text-center">
    <div class="flex items-center justify-center w-16 h-16 border-2 border-black bg-white shadow-drag mb-6">
        <span class="material-symbols-outlined text-black" style="font-size: 32px;">
            {{ $icon ?? 'inbox' }}
        </span>
    </div>
    <h3 class="text-lg font-bold text-text-main tracking-tight mb-2">{{ $title ?? 'Belum ada data' }}</h3>
    <p class="text-[14px] text-text-muted font-medium max-w-sm">{{ $message ?? 'Tidak ada item yang tersedia saat ini.' }}</p>
</div>
