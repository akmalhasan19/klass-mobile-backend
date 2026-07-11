@extends('admin.layouts.app')

@section('title', 'Detail Pengguna')
@section('page-title', 'Detail Pengguna')
@section('page-description', 'Informasi lengkap dan manajemen role pengguna.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">

        <div class="flex items-center justify-between">
            <a href="{{ route('admin.users.index') }}" class="inline-flex items-center gap-2 px-6 py-3 bg-white border-2 border-black font-bold text-[13px] shadow-drag hover:bg-gray-50 transition-all active:translate-x-[2px] active:translate-y-[2px] active:shadow-none">
                <span class="material-symbols-outlined" style="font-size: 20px;">arrow_back</span>
                KEMBALI KE DAFTAR USER
            </a>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-3 gap-8">
            {{-- Left Col: User Info --}}
            <div class="lg:col-span-1 space-y-6">
                <div class="bg-surface border-2 border-black p-8 shadow-drag">
                    <div class="flex justify-center mb-8">
                        @if($user->avatar_url)
                            <div class="relative">
                                <img src="{{ $user->avatar_url }}" alt="Avatar" class="w-32 h-32 border-2 border-black object-cover shadow-drag">
                                <div class="absolute -bottom-2 -right-2 bg-primary border-2 border-black p-1 shadow-[2px_2px_0px_rgba(0,0,0,1)]">
                                    <span class="material-symbols-outlined text-black" style="font-size: 20px;">photo_camera</span>
                                </div>
                            </div>
                        @else
                            <div class="w-32 h-32 border-2 border-black bg-white flex items-center justify-center text-4xl font-bold text-text-main shadow-drag">
                                {{ substr($user->name, 0, 1) }}
                            </div>
                        @endif
                    </div>
                    <h3 class="text-xl font-bold text-center text-text-main tracking-tight">{{ $user->name }}</h3>
                    <p class="text-[14px] text-center text-text-muted mt-1 font-medium">{{ $user->email }}</p>
                    
                    <div class="mt-10 space-y-4 text-[13px] divide-y-2 divide-black/5">
                        <div class="flex justify-between pb-4">
                            <span class="text-text-muted font-bold uppercase mono-text text-[11px] tracking-wider">Status Akun</span>
                            <span class="inline-flex items-center gap-1.5 text-emerald-600 font-bold">
                                <span class="w-2 h-2 bg-emerald-500 border border-black shadow-[1px_1px_0px_rgba(0,0,0,1)]"></span>
                                AKTIF
                            </span>
                        </div>
                        <div class="flex justify-between py-4">
                            <span class="text-text-muted font-bold uppercase mono-text text-[11px] tracking-wider">Terdaftar</span>
                            <span class="text-text-main font-bold">{{ $user->created_at->format('d M Y H:i') }}</span>
                        </div>
                        <div class="flex justify-between py-4">
                            <span class="text-text-muted font-bold uppercase mono-text text-[11px] tracking-wider">ID Sistem</span>
                            <span class="text-text-main font-mono text-[11px] bg-gray-50 border border-black/10 px-1.5 py-0.5">{{ $user->id }}</span>
                        </div>
                        <div class="flex justify-between pt-4">
                            <span class="text-text-muted font-bold uppercase mono-text text-[11px] tracking-wider">Role</span>
                            <span class="inline-flex items-center px-2.5 py-1 border border-black text-[10px] font-bold uppercase tracking-wider shadow-[2px_2px_0px_rgba(0,0,0,1)] {{ $user->isAdmin() ? 'bg-primary text-black' : 'bg-gray-100 text-text-muted' }}">
                                {{ $user->role }}
                            </span>
                        </div>
                    </div>
                </div>
            </div>

            {{-- Right Col: Management Actions --}}
            <div class="lg:col-span-2 space-y-8">
                <div class="bg-surface border-2 border-black p-8 shadow-drag">
                    <div class="flex items-center gap-3 mb-8 border-b-2 border-black pb-4">
                        <span class="material-symbols-outlined text-primary" style="font-size: 28px;">admin_panel_settings</span>
                        <h3 class="text-lg font-bold text-text-main tracking-tight uppercase tracking-wider">Kontrol Hak Akses</h3>
                    </div>
                    
                    <form action="{{ route('admin.users.update-role', $user->id) }}" method="POST" class="space-y-8">
                        @csrf
                        @method('PATCH')
                        
                        <div class="space-y-3">
                            <label for="role" class="block text-[12px] font-bold uppercase mono-text text-text-muted tracking-wide">Pilih Role Baru</label>
                            <div class="relative">
                                <select id="role" name="role" class="bg-white border-2 border-black text-text-main text-[14px] font-bold focus:ring-primary focus:border-primary block w-full p-4 appearance-none">
                                    <option value="user" {{ $user->role === 'user' ? 'selected' : '' }}>User (Pengguna Aplikasi Mobile)</option>
                                    <option value="admin" {{ $user->role === 'admin' ? 'selected' : '' }}>Admin (Pengawas & Back-Office)</option>
                                </select>
                                <div class="absolute inset-y-0 right-0 flex items-center pr-4 pointer-events-none">
                                    <span class="material-symbols-outlined">unfold_more</span>
                                </div>
                            </div>
                            <div class="bg-amber-50 border-2 border-black p-5 shadow-[4px_4px_0px_#000000] mt-6">
                                <div class="flex gap-3">
                                    <span class="material-symbols-outlined text-amber-600">warning</span>
                                    <div>
                                        <p class="font-bold text-[13px] text-amber-900 uppercase tracking-wider mb-1">Peringatan Keamanan</p>
                                        <p class="text-[12px] text-amber-800 font-medium leading-relaxed">
                                            Memberikan role "Admin" akan memberikan akses penuh ke seluruh data monitoring dan pengaturan sistem. Pastikan tindakan ini telah disetujui oleh tim terkait.
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div class="pt-2 flex justify-end">
                            <button type="submit" class="bg-primary hover:bg-[#0da673] text-black font-extrabold border-2 border-black px-10 py-4 shadow-drag transition-all active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase text-[14px] tracking-widest">
                                UPDATE HAK AKSES
                            </button>
                        </div>
                    </form>
                </div>
                
                <div class="bg-surface border-2 border-black p-8 shadow-drag relative overflow-hidden group">
                    <div class="absolute top-0 right-0 w-24 h-24 bg-gray-50 border-bl-2 border-black translate-x-12 -translate-y-12 rotate-45 group-hover:bg-primary transition-colors"></div>
                    <h3 class="text-lg font-bold text-text-main tracking-tight mb-4 uppercase tracking-wider relative z-10">Histori Aktivitas</h3>
                    <p class="text-[14px] text-text-muted font-medium leading-relaxed mb-6 relative z-10">
                        Ingin melihat tindakan apa saja yang telah dilakukan oleh user ini atau perubahan apa yang terjadi padanya?
                    </p>
                    <a href="{{ route('admin.activity-logs.index', ['search' => $user->email]) }}" class="inline-flex items-center gap-2 font-bold text-[13px] uppercase tracking-wider border-b-4 border-primary hover:bg-primary/10 px-1 py-1 transition-all">
                        LIHAT ACTIVITY LOGS
                        <span class="material-symbols-outlined" style="font-size: 18px;">history</span>
                    </a>
                </div>
            </div>
        </div>
    </div>
</div>
@endsection
