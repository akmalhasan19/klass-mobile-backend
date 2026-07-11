@extends('admin.layouts.app')

@section('title', 'Edit Topik')
@section('page-title', 'Edit Topik')
@section('page-description', 'Ubah detail dan status visibilitas topik.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">

        <div class="flex items-center justify-between">
            <a href="{{ route('admin.topics.index') }}" class="inline-flex items-center gap-2 font-bold text-black border-b-2 border-black hover:bg-gray-100 transition-colors px-1 text-[13px] uppercase tracking-wider">
                <span class="material-symbols-outlined" style="font-size: 18px;">arrow_back</span>
                Kembali ke Daftar Topik
            </a>
        </div>

        <div class="bg-surface border-2 border-black shadow-drag max-w-2xl">
            <div class="px-6 py-4 border-b-2 border-black bg-gray-50">
                <h3 class="text-[13px] font-bold uppercase tracking-wider text-text-main flex items-center gap-2">
                    <span class="material-symbols-outlined" style="font-size: 18px;">edit_note</span>
                    Informasi Topik
                </h3>
            </div>
            
            <form action="{{ route('admin.topics.update', $topic->id) }}" method="POST" class="p-8 space-y-6">
                @csrf
                @method('PATCH')
                
                <div>
                    <label for="title" class="block text-[11px] mono-text font-bold text-text-muted uppercase tracking-wider mb-2">Judul Topik</label>
                    <input type="text" id="title" name="title" value="{{ old('title', $topic->title) }}" class="bg-white border-2 border-black text-text-main text-[14px] font-medium focus:ring-primary focus:border-primary block w-full p-3" required>
                    @error('title')
                        <p class="mt-2 text-xs font-bold text-red-600 uppercase tracking-tight">{{ $message }}</p>
                    @enderror
                </div>

                <div>
                    <label for="is_published" class="flex items-center gap-3 cursor-pointer group">
                        <div class="relative">
                            <input type="checkbox" id="is_published" name="is_published" value="1" class="peer appearance-none w-6 h-6 border-2 border-black bg-white checked:bg-primary transition shadow-[2px_2px_0px_rgba(0,0,0,1)]" {{ old('is_published', $topic->is_published) ? 'checked' : '' }}>
                            <span class="material-symbols-outlined absolute inset-0 flex items-center justify-center text-black opacity-0 peer-checked:opacity-100 pointer-events-none" style="font-size: 18px;">check</span>
                        </div>
                        <span class="text-[13px] font-bold text-text-main uppercase tracking-tight group-hover:text-black transition-colors">Publish (Tampilkan ke pengguna akhir)</span>
                    </label>
                </div>

                <div class="pt-6 flex justify-end gap-4 border-t-2 border-black/10">
                    <a href="{{ route('admin.topics.index') }}" class="inline-flex items-center justify-center bg-white border-2 border-black px-6 py-3 font-bold text-[13px] uppercase tracking-wider hover:bg-gray-50 transition">
                        BATAL
                    </a>
                    <button type="submit" class="inline-flex items-center justify-center bg-black hover:bg-gray-800 text-white font-bold px-8 py-3 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase tracking-wider text-[13px]">
                        SIMPAN PERUBAHAN
                    </button>
                </div>
            </form>
        </div>
    </div>
</div>
@endsection

