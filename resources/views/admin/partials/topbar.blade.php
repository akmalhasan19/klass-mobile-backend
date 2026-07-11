<header class="px-8 py-6 border-b border-border bg-surface shrink-0 z-10 w-full">
    <div class="max-w-[1200px] mx-auto flex items-end justify-between">
        <div>
            <h2 class="text-2xl font-bold tracking-tight text-text-main">@yield('page-title', 'Dashboard')</h2>
            @hasSection('page-description')
                <p class="text-[13px] text-text-muted mt-1 font-medium">@yield('page-description')</p>
            @endif
        </div>
        <div class="flex items-center gap-4">
            <span id="local-time-display" class="text-[11px] uppercase tracking-wider text-text-muted mono-text font-medium flex items-center gap-2">
                <!-- Time will be injected by JS -->
            </span>
            @hasSection('topbar-actions')
                @yield('topbar-actions')
            @endif
        </div>
    </div>
</header>

@push('scripts')
<script>
    function updateLocalTime() {
        const timeDisplay = document.getElementById('local-time-display');
        if (!timeDisplay) return;

        const now = new Date();
        const dateOptions = { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' };
        const timeOptions = { hour: '2-digit', minute: '2-digit', hour12: false };
        
        const dateStr = now.toLocaleDateString('id-ID', dateOptions);
        const timeStr = now.toLocaleTimeString('id-ID', timeOptions).replace('.', ':');
        
        const offset = -now.getTimezoneOffset() / 60;
        let tzName = '';
        if (offset === 7) tzName = 'WIB';
        else if (offset === 8) tzName = 'WITA';
        else if (offset === 9) tzName = 'WIT';
        else tzName = `GMT${offset >= 0 ? '+' : ''}${offset}`;
        
        timeDisplay.textContent = `${dateStr} · ${timeStr} ${tzName}`;
    }

    // Update immediately, then every minute
    updateLocalTime();
    setInterval(updateLocalTime, 60000);
</script>
@endpush
