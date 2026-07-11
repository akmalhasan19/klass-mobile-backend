{{--
    Admin Flash Messages
    Mendukung session keys: success, error, warning, info
    Auto-dismiss dengan vanilla JS setelah 5 detik.
--}}
@if(session('success') || session('error') || session('warning') || session('info'))
<div id="admin-flash" class="px-6 pt-4 space-y-2 transition-all duration-300">

    @if(session('success'))
    <div class="admin-flash-item flex items-start gap-3 p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-emerald-300">
        <svg class="w-5 h-5 mt-0.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <span class="text-sm flex-1">{{ session('success') }}</span>
        <button onclick="this.closest('.admin-flash-item').remove()" class="shrink-0 text-emerald-400/60 hover:text-emerald-300 transition-colors">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
        </button>
    </div>
    @endif

    @if(session('error'))
    <div class="admin-flash-item flex items-start gap-3 p-4 rounded-xl bg-red-500/10 border border-red-500/20 text-red-300">
        <svg class="w-5 h-5 mt-0.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
        </svg>
        <span class="text-sm flex-1">{{ session('error') }}</span>
        <button onclick="this.closest('.admin-flash-item').remove()" class="shrink-0 text-red-400/60 hover:text-red-300 transition-colors">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
        </button>
    </div>
    @endif

    @if(session('warning'))
    <div class="admin-flash-item flex items-start gap-3 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-300">
        <svg class="w-5 h-5 mt-0.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
        </svg>
        <span class="text-sm flex-1">{{ session('warning') }}</span>
        <button onclick="this.closest('.admin-flash-item').remove()" class="shrink-0 text-amber-400/60 hover:text-amber-300 transition-colors">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
        </button>
    </div>
    @endif

    @if(session('info'))
    <div class="admin-flash-item flex items-start gap-3 p-4 rounded-xl bg-blue-500/10 border border-blue-500/20 text-blue-300">
        <svg class="w-5 h-5 mt-0.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M11.25 11.25l.041-.02a.75.75 0 011.063.852l-.708 2.836a.75.75 0 001.063.853l.041-.021M21 12a9 9 0 11-18 0 9 9 0 0118 0zm-9-3.75h.008v.008H12V8.25z" />
        </svg>
        <span class="text-sm flex-1">{{ session('info') }}</span>
        <button onclick="this.closest('.admin-flash-item').remove()" class="shrink-0 text-blue-400/60 hover:text-blue-300 transition-colors">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
        </button>
    </div>
    @endif

</div>

<script>
    // Auto-dismiss flash setelah 5 detik
    setTimeout(function () {
        const flash = document.getElementById('admin-flash');
        if (flash) {
            flash.style.opacity = '0';
            flash.style.transform = 'translateY(-4px)';
            setTimeout(() => flash.remove(), 300);
        }
    }, 5000);
</script>
@endif
