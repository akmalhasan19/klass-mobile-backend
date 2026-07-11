<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <title>@yield('title', 'Dashboard') — Klass Admin</title>
    <meta name="description" content="Panel administrasi Klass — kelola pengguna, konten, marketplace, dan pengaturan aplikasi.">

    {{-- Google Fonts & Material Symbols --}}
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
    <link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:wght,FILL@100..700,0..1&display=swap" rel="stylesheet" />

    @vite(['resources/css/app.css', 'resources/js/app.js'])
</head>
<body class="flex h-screen overflow-hidden antialiased bg-[#F9FAFB]">

    <!-- Sidebar (Shared Component) -->
    @include('admin.partials.sidebar')

    <!-- Main Content Area -->
    <main class="flex-1 flex flex-col h-full bg-[#F9FAFB] relative overflow-hidden">
        {{-- Topbar --}}
        @include('admin.partials.topbar')

        {{-- Flash Messages --}}
        <div class="px-8 mt-4 z-10 shrink-0">
            @include('admin.partials.flash')
        </div>

        {{-- Page Content --}}
        <div class="flex-1 overflow-hidden">
            @yield('content')
        </div>
    </main>

    @stack('scripts')
</body>
</html>
