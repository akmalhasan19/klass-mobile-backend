<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <title>Login Admin — Klass</title>
    <meta name="description" content="Login ke panel administrasi Klass.">
    <meta name="robots" content="noindex, nofollow">

    {{-- Google Fonts & Material Symbols --}}
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
    <link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:wght,FILL@100..700,0..1&display=swap" rel="stylesheet" />

    @vite(['resources/css/app.css', 'resources/js/app.js'])
</head>
<body class="min-h-screen font-['Inter',sans-serif] antialiased bg-[#F9FAFB] text-text-main flex flex-col items-center justify-center p-4 selection:bg-primary selection:text-black">

    <div class="w-full max-w-[400px]">
        {{-- Header / Logo Area --}}
        <div class="mb-8 text-center">
            <div class="inline-flex items-center justify-center w-16 h-16 bg-primary border-2 border-black shadow-drag mb-5">
                <span class="material-symbols-outlined text-[32px] text-black">admin_panel_settings</span>
            </div>
            <h1 class="text-3xl font-extrabold text-black tracking-tight font-display uppercase">Klass Admin</h1>
            <p class="text-[13px] mono-text text-text-muted mt-2 tracking-wide uppercase">System Access Portal</p>
        </div>

        {{-- Login Card --}}
        <div class="bg-surface border-2 border-black p-8 shadow-drag relative">

            {{-- Flash error dari session --}}
            @if(session('error'))
            <div class="flex items-start gap-3 p-4 mb-6 bg-red-50 border-2 border-red-500 text-red-700">
                <span class="material-symbols-outlined text-red-500 shrink-0">error</span>
                <p class="text-[13px] font-medium mt-0.5">{{ session('error') }}</p>
            </div>
            @endif

            {{-- Form --}}
            <form method="POST" action="{{ route('admin.login.post') }}" id="admin-login-form" class="space-y-5">
                @csrf

                {{-- Email --}}
                <div>
                    <label for="email" class="block text-[11px] mono-text font-semibold uppercase tracking-wider text-text-main mb-2">
                        Email Address
                    </label>
                    <input
                        id="email"
                        name="email"
                        type="email"
                        autocomplete="email"
                        required
                        value="{{ old('email') }}"
                        placeholder="admin@klass.id"
                        class="w-full px-4 py-2.5 bg-surface border-2 {{ $errors->has('email') ? 'border-red-500 focus:border-red-500' : 'border-border focus:border-black' }} text-text-main placeholder:text-text-muted focus:outline-none focus:ring-0 transition-colors text-[14px]"
                    >
                    @error('email')
                    <p class="mt-2 text-[11px] mono-text text-red-500 font-medium">{{ $message }}</p>
                    @enderror
                </div>

                {{-- Password --}}
                <div>
                    <label for="password" class="block text-[11px] mono-text font-semibold uppercase tracking-wider text-text-main mb-2">
                        Password
                    </label>
                    <div class="relative">
                        <input
                            id="password"
                            name="password"
                            type="password"
                            autocomplete="current-password"
                            required
                            placeholder="••••••••"
                            class="w-full px-4 py-2.5 pr-12 bg-surface border-2 {{ $errors->has('password') ? 'border-red-500 focus:border-red-500' : 'border-border focus:border-black' }} text-text-main placeholder:text-text-muted focus:outline-none focus:ring-0 transition-colors text-[14px]"
                        >
                        {{-- Toggle password visibility --}}
                        <button type="button" id="toggle-password"
                            class="absolute right-3 top-1/2 -translate-y-1/2 text-text-muted hover:text-black transition-colors focus:outline-none flex items-center justify-center w-8 h-8">
                            <span id="eye-open" class="material-symbols-outlined text-[20px]">visibility</span>
                            <span id="eye-closed" class="material-symbols-outlined text-[20px] hidden">visibility_off</span>
                        </button>
                    </div>
                    @error('password')
                    <p class="mt-2 text-[11px] mono-text text-red-500 font-medium">{{ $message }}</p>
                    @enderror
                </div>

                {{-- Remember me --}}
                <div class="flex items-center gap-3 pt-2">
                    <div class="relative flex items-start">
                        <div class="flex h-5 items-center">
                            <input id="remember" name="remember" type="checkbox"
                                class="w-4 h-4 rounded-none border-2 border-border text-primary bg-surface focus:ring-0 focus:ring-offset-0 checked:border-black checked:bg-black transition-colors cursor-pointer">
                        </div>
                        <div class="ml-2">
                            <label for="remember" class="text-[12px] font-medium text-text-main cursor-pointer select-none">Ingat sesi saya</label>
                        </div>
                    </div>
                </div>

                {{-- Submit --}}
                <div class="pt-2">
                    <button
                        id="submit-btn"
                        type="submit"
                        class="w-full flex items-center justify-center gap-2 px-4 py-3 bg-primary border-2 border-black text-black text-[14px] font-bold uppercase tracking-wide hover:bg-[#0e9f6e] focus:outline-none focus:ring-0 transition-all shadow-drag hover:shadow-none hover:translate-x-[4px] hover:translate-y-[4px] active:bg-[#0c8a5f]">
                        <span class="material-symbols-outlined text-[20px]">login</span>
                        <span>Otorisasi Masuk</span>
                    </button>
                </div>
            </form>
        </div>

        {{-- Footer --}}
        <div class="mt-8 text-center">
            <p class="text-[11px] mono-text text-text-muted uppercase tracking-widest">
                © {{ date('Y') }} Klass. Restricted Access.
            </p>
        </div>
    </div>

    <script>
        // Toggle password visibility
        const toggleBtn = document.getElementById('toggle-password');
        const pwInput   = document.getElementById('password');
        const eyeOpen   = document.getElementById('eye-open');
        const eyeClosed = document.getElementById('eye-closed');

        toggleBtn?.addEventListener('click', () => {
            const isHidden = pwInput.type === 'password';
            pwInput.type = isHidden ? 'text' : 'password';
            eyeOpen.classList.toggle('hidden', isHidden);
            eyeClosed.classList.toggle('hidden', !isHidden);
        });

        // Form submission loading state
        document.getElementById('admin-login-form')?.addEventListener('submit', function () {
            const btn = document.getElementById('submit-btn');
            const span = btn.querySelector('span:not(.material-symbols-outlined)');
            const icon = btn.querySelector('.material-symbols-outlined');
            
            btn.disabled = true;
            btn.classList.add('opacity-80', 'cursor-not-allowed', 'shadow-none', 'translate-x-[4px]', 'translate-y-[4px]');
            btn.classList.remove('hover:translate-x-[4px]', 'hover:translate-y-[4px]', 'hover:shadow-none');
            
            if (span) span.innerText = 'MEMPROSES...';
            if (icon) icon.innerText = 'sync';
            if (icon) icon.classList.add('animate-spin');
        });
    </script>
</body>
</html>