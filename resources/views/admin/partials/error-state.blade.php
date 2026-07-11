{{--
    Admin Error State
    Usage: @include('admin.partials.error-state', [
        'message' => 'Gagal memuat data pengguna.',
        'retry'   => url()->current(),  // optional retry URL
    ])
--}}
<div class="flex flex-col items-center justify-center py-16 text-center">
    <div class="flex items-center justify-center w-14 h-14 rounded-2xl bg-red-500/10 mb-4">
        <svg class="w-7 h-7 text-red-400" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
        </svg>
    </div>
    <h3 class="text-sm font-semibold text-slate-300 mb-1">Terjadi kesalahan</h3>
    <p class="text-sm text-slate-500 max-w-xs mb-4">{{ $message ?? 'Gagal memuat data. Silakan coba lagi.' }}</p>
    @if(!empty($retry))
    <a href="{{ $retry }}"
        class="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-slate-300 bg-slate-800 border border-slate-700 rounded-lg hover:bg-slate-700 transition-colors">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99" />
        </svg>
        Coba lagi
    </a>
    @endif
</div>
