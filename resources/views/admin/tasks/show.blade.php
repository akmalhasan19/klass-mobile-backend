@extends('admin.layouts.app')

@section('title', 'Tinjauan Task')
@section('page-title', 'Detail & Cek Task')
@section('page-description', 'Tinjau metadata task dan berikan moderasi apabila bermasalah.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">

        <div class="flex items-center justify-between">
            <a href="{{ route('admin.tasks.index') }}" class="inline-flex items-center gap-2 px-6 py-3 bg-white border-2 border-black font-bold text-[13px] shadow-drag hover:bg-gray-50 transition-all active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase">
                <span class="material-symbols-outlined" style="font-size: 20px;">arrow_back</span>
                KEMBALI KE DAFTAR TASKS
            </a>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
            
            {{-- Info Card --}}
            <div class="bg-surface border-2 border-black p-8 shadow-drag space-y-6">
                <div class="flex items-center gap-3 mb-2 border-b-2 border-black pb-4">
                    <span class="material-symbols-outlined text-primary" style="font-size: 28px;">assignment</span>
                    <h3 class="text-xl font-bold text-text-main tracking-tight uppercase">Data Marketplace Task</h3>
                </div>

                <div class="space-y-6 text-[13px]">
                    <div>
                        <span class="block text-[11px] font-bold text-text-muted uppercase tracking-widest mono-text mb-2">ID Tugas</span>
                        <p class="font-mono text-text-main bg-gray-50 border border-black/10 p-3 break-all">{{ $task->id }}</p>
                    </div>

                    <div>
                        <span class="block text-[11px] font-bold text-text-muted uppercase tracking-widest mono-text mb-2">Konten Pemilik (Parent)</span>
                        @if($task->content)
                        <div class="bg-gray-50 border-2 border-black p-4 shadow-[4px_4px_0px_#000000]">
                            <p class="text-[15px] text-text-main font-bold mb-1">{{ $task->content->title }}</p>
                            <p class="text-[12px] text-text-muted font-medium uppercase tracking-wider">TOPIK: {{ $task->content->topic?->title ?? '-' }}</p>
                        </div>
                        @else
                        <div class="bg-red-50 border-2 border-red-200 p-4 text-red-600 font-bold italic">
                            Konten induk telah dihapus oleh User / Admin sebelumnya.
                        </div>
                        @endif
                    </div>

                    <div class="grid grid-cols-2 gap-4">
                        <div>
                            <span class="block text-[11px] font-bold text-text-muted uppercase tracking-widest mono-text mb-2">Waktu Dibuat</span>
                            <p class="text-text-main font-bold">{{ $task->created_at->format('d F Y') }}</p>
                            <p class="text-[11px] text-text-muted mono-text">{{ $task->created_at->format('H:i:s') }}</p>
                        </div>
                        <div>
                            <span class="block text-[11px] font-bold text-text-muted uppercase tracking-widest mono-text mb-2">Status Saat Ini</span>
                             <div class="mt-1">
                                @php
                                    $statusColor = match($task->status) {
                                        'open' => 'bg-emerald-500 text-white',
                                        'taken', 'in_progress' => 'bg-blue-500 text-white',
                                        'completed', 'verified' => 'bg-gray-100 text-gray-700',
                                        default => 'bg-white text-gray-400',
                                    };
                                @endphp
                                <span class="inline-flex items-center px-2.5 py-1 border-2 border-black text-[10px] font-bold uppercase tracking-wider shadow-[2px_2px_0px_rgba(0,0,0,1)] {{ $statusColor }}">
                                    {{ $task->status ?? 'UNKNOWN' }}
                                </span>
                             </div>
                        </div>
                    </div>

                    @if($task->attachment_url)
                    <div class="pt-4 border-t-2 border-black/5">
                        <span class="block text-[11px] font-bold text-text-muted uppercase tracking-widest mono-text mb-3">Attachment Upload</span>
                        <a href="{{ $task->attachment_url }}" target="_blank" class="inline-flex items-center gap-3 px-6 py-3 bg-white border-2 border-black font-bold text-[13px] shadow-[4px_4px_0px_#000000] hover:bg-primary transition-all active:translate-x-[2px] active:translate-y-[2px] active:shadow-none">
                            <span class="material-symbols-outlined">description</span>
                            BUKA FILE LAMPIRAN
                        </a>
                    </div>
                    @endif
                </div>
            </div>

            {{-- Administration Actions --}}
            <div class="space-y-8">
                
                {{-- Update Status --}}
                <div class="bg-surface border-2 border-black p-8 shadow-drag">
                    <div class="flex items-center gap-3 mb-6 border-b-2 border-black pb-4">
                        <span class="material-symbols-outlined text-amber-500" style="font-size: 26px;">published_with_changes</span>
                        <h3 class="text-lg font-bold text-text-main uppercase tracking-wider">Override Status</h3>
                    </div>
                    
                    <form action="{{ route('admin.tasks.update-status', $task->id) }}" method="POST" class="space-y-6">
                        @csrf
                        @method('PATCH')
                        
                        <div class="space-y-3">
                            <label for="status" class="block text-[11px] font-bold uppercase mono-text text-text-muted tracking-wide">Pilih Status Baru</label>
                            <div class="relative">
                                <select id="status" name="status" class="bg-white border-2 border-black text-text-main text-[14px] font-bold focus:ring-primary focus:border-primary block w-full p-4 appearance-none">
                                    <option value="open" {{ $task->status == 'open' ? 'selected' : '' }}>Open (Tersedia)</option>
                                    <option value="taken" {{ $task->status == 'taken' ? 'selected' : '' }}>Taken (Diambil)</option>
                                    <option value="in_progress" {{ $task->status == 'in_progress' ? 'selected' : '' }}>In Progress (Pengerjaan)</option>
                                    <option value="completed" {{ $task->status == 'completed' ? 'selected' : '' }}>Completed (Selesai)</option>
                                    <option value="verified" {{ $task->status == 'verified' ? 'selected' : '' }}>Verified (Terverifikasi)</option>
                                </select>
                                <div class="absolute inset-y-0 right-0 flex items-center pr-4 pointer-events-none">
                                    <span class="material-symbols-outlined">expand_more</span>
                                </div>
                            </div>
                        </div>

                        <div class="flex justify-end">
                            <button type="submit" class="bg-black hover:bg-gray-800 text-white font-extrabold px-8 py-3 shadow-drag transition-all active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase tracking-widest text-[13px]">
                                UPDATE STATUS
                            </button>
                        </div>
                    </form>
                </div>

                {{-- Delete Moderation --}}
                <div class="bg-red-50 border-2 border-red-600 p-8 shadow-[8px_8px_0px_#ce2c2c] relative overflow-hidden">
                    <div class="absolute top-0 right-0 w-16 h-16 bg-red-600 rotate-45 translate-x-8 -translate-y-8"></div>
                    <div class="flex items-center gap-3 mb-4">
                        <span class="material-symbols-outlined text-red-600" style="font-size: 28px;">dangerous</span>
                        <h3 class="text-lg font-bold text-red-900 uppercase tracking-wider">Moderasi Sepihak</h3>
                    </div>
                    <p class="text-[13px] text-red-800 font-medium leading-relaxed mb-6">
                        Gunakan aksi ini jika Task melanggar S&K, mengandung eksploitasi, atau data tidak wajar. <span class="font-bold underline">Penghapusan bersifat permanen</span> dan tidak dapat dibatalkan.
                    </p>

                    <form action="{{ route('admin.tasks.destroy', $task->id) }}" method="POST" onsubmit="return confirm('Peringatan: Anda akan menghapus task ini secara permanen. Pengguna tidak akan dapat mengaksesnya kembali. Yakin ingin melanjutkan?');">
                        @csrf
                        @method('DELETE')
                        <button type="submit" class="bg-white hover:bg-red-600 hover:text-white text-red-600 border-4 border-red-600 font-black py-4 transition-all w-full shadow-drag uppercase tracking-[0.2em] text-[14px]">
                            HAPUS PERMANEN TASK
                        </button>
                    </form>
                </div>

            </div>

        </div>
    </div>
</div>
@endsection
