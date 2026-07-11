@extends('admin.layouts.app')

@section('title', 'Manage Users')
@section('page-title', 'User Management')
@section('page-description', 'Cari pengguna dan kelola hak akses sistem.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">

        {{-- Filter & Search --}}
        <div class="bg-surface border-2 border-black p-6 shadow-drag">
            <form method="GET" action="{{ route('admin.users.index') }}" class="flex flex-col md:flex-row gap-4">
                <div class="flex-1 relative">
                    <div class="absolute inset-y-0 left-0 flex items-center pl-4 pointer-events-none text-text-muted">
                        <span class="material-symbols-outlined" style="font-size: 20px;">search</span>
                    </div>
                    <input type="text" name="search" value="{{ $search }}" placeholder="Cari nama atau email pengguna..." class="bg-white border-2 border-black text-text-main text-[14px] font-medium focus:ring-primary focus:border-primary block w-full pl-12 p-3">
                </div>
                <div class="flex gap-3">
                    <button type="submit" class="bg-black hover:bg-gray-800 text-white font-bold px-8 py-3 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none">
                        CARI
                    </button>
                    @if(request()->filled('search'))
                        <a href="{{ route('admin.users.index') }}" class="flex items-center justify-center bg-white border-2 border-black px-6 py-3 font-bold hover:bg-gray-50 transition">
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
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Pengguna</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Role</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Terdaftar Pada</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-right">Aksi</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-black/10 text-text-main">
                        @forelse($users as $user)
                        <tr class="hover:bg-gray-50 transition-colors group">
                            <td class="px-6 py-5">
                                <div class="flex items-center gap-4">
                                    @if($user->avatar_url)
                                        <img src="{{ $user->avatar_url }}" alt="Avatar" class="w-10 h-10 border border-black object-cover shadow-[2px_2px_0px_rgba(0,0,0,1)]">
                                    @else
                                        <div class="w-10 h-10 border border-black bg-white flex items-center justify-center font-bold text-[14px] shadow-[2px_2px_0px_rgba(0,0,0,1)]">
                                            {{ substr($user->name, 0, 1) }}
                                        </div>
                                    @endif
                                    <div>
                                        <div class="font-bold text-[14px]">{{ $user->name }}</div>
                                        <div class="text-[11px] mono-text text-text-muted mt-0.5">{{ $user->email }}</div>
                                    </div>
                                </div>
                            </td>
                            <td class="px-6 py-5">
                                <span class="inline-flex items-center px-2 py-0.5 border border-black text-[10px] font-bold uppercase tracking-wider shadow-[2px_2px_0px_rgba(0,0,0,1)] {{ $user->isAdmin() ? 'bg-primary text-black' : 'bg-gray-100 text-text-muted' }}">
                                    {{ $user->role }}
                                </span>
                            </td>
                            <td class="px-6 py-5 mono-text text-text-muted">
                                {{ $user->created_at->format('d M Y') }}
                            </td>
                            <td class="px-6 py-5 text-right">
                                <a href="{{ route('admin.users.show', $user->id) }}" class="inline-flex items-center gap-1 font-bold text-black border-b-2 border-primary hover:bg-primary/10 transition-colors px-1">
                                    DETAIL
                                    <span class="material-symbols-outlined" style="font-size: 16px;">arrow_forward</span>
                                </a>
                            </td>
                        </tr>
                        @empty
                        <tr>
                            <td colspan="4" class="py-12 bg-white">
                                @include('admin.partials.empty-state', [
                                    'title'   => 'User tidak ditemukan',
                                    'message' => 'Coba gunakan kata kunci pencarian yang berbeda.',
                                    'icon'    => 'person_off'
                                ])
                            </td>
                        </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>
            
            {{-- Pagination --}}
            @if($users->hasPages())
            <div class="px-6 py-6 border-t-2 border-black bg-gray-50">
                {{ $users->links() }}
            </div>
            @endif
        </div>
    </div>
</div>
@endsection
