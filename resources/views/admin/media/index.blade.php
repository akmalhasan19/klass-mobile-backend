@extends('admin.layouts.app')

@section('title', 'Media Library')
@section('page-title', 'Media Library')
@section('page-description', 'Kelola semua aset visual dan dokumen sistem dalam satu tempat.')

@section('content')
<div class="h-full p-8 overflow-y-auto scrollbar-thin" id="media-container">
    <div class="max-w-[1400px] mx-auto space-y-8 pb-32">

        {{-- Header Actions & Filters --}}
        <div class="flex flex-col lg:flex-row gap-6 items-start lg:items-end justify-between">
            <div class="flex-1 w-full space-y-4">
                {{-- Search & Filters --}}
                <div class="bg-surface border-2 border-black p-6 shadow-drag">
                    <form method="GET" action="{{ route('admin.media.index') }}" class="grid grid-cols-1 md:grid-cols-4 gap-4">
                        <div class="md:col-span-2 relative">
                            <div class="absolute inset-y-0 left-0 flex items-center pl-4 pointer-events-none text-text-muted">
                                <span class="material-symbols-outlined" style="font-size: 20px;">search</span>
                            </div>
                            <input type="text" name="search" value="{{ $search }}" placeholder="Cari nama file..." class="bg-white border-2 border-black text-text-main text-[14px] font-medium focus:ring-primary focus:border-primary block w-full pl-12 p-3">
                        </div>
                        
                        <div>
                            <select name="type" class="bg-white border-2 border-black text-text-main text-[14px] font-bold focus:ring-primary focus:border-primary block w-full p-3 uppercase">
                                <option value="">Semua Tipe</option>
                                <option value="image" {{ $type == 'image' ? 'selected' : '' }}>Gambar</option>
                                <option value="video" {{ $type == 'video' ? 'selected' : '' }}>Video</option>
                                <option value="application" {{ $type == 'application' ? 'selected' : '' }}>Dokumen</option>
                            </select>
                        </div>

                        <div class="flex gap-2">
                            <button type="submit" class="flex-1 bg-black hover:bg-gray-800 text-white font-bold py-3 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase">
                                Cari
                            </button>
                            @if(request()->anyFilled(['search', 'type', 'category']))
                                <a href="{{ route('admin.media.index') }}" class="flex items-center justify-center bg-white border-2 border-black px-4 font-bold hover:bg-gray-50 transition border-l-0">
                                    <span class="material-symbols-outlined">restart_alt</span>
                                </a>
                            @endif
                        </div>
                    </form>
                </div>
            </div>

            <div class="flex gap-4 w-full lg:w-auto">
                {{-- Selection Toggle --}}
                <button id="toggle-selection" class="flex-1 lg:flex-none flex items-center justify-center gap-2 bg-white border-2 border-black px-6 py-3 font-bold hover:bg-gray-50 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none">
                    <span class="material-symbols-outlined" id="selection-icon">checklist</span>
                    <span id="selection-text">PILIH MASSAL</span>
                </button>

                {{-- Upload Button --}}
                <button onclick="openUploadModal()" class="flex-1 lg:flex-none flex items-center justify-center gap-2 bg-primary border-2 border-black px-8 py-3 font-bold text-black hover:bg-primary-dark transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase">
                    <span class="material-symbols-outlined">upload_file</span>
                    Unggah Media
                </button>
            </div>
        </div>

        {{-- Media Grid --}}
        <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-6">
            @forelse($medias as $media)
                @php
                    $isImage = str_contains($media->mime_type, 'image/');
                    $url = $media->url;
                    
                    // Simple size formatter
                    $size = $media->size < 1024 * 1024 
                            ? round($media->size / 1024, 1) . ' KB' 
                            : round($media->size / (1024 * 1024), 1) . ' MB';
                @endphp
                
                <div class="media-card group relative bg-white border-2 border-black shadow-drag hover:-translate-y-1 hover:-translate-x-1 hover:shadow-[8px_8px_0px_0px_rgba(0,0,0,1)] transition-all duration-200 overflow-hidden flex flex-col cursor-pointer" 
                     data-id="{{ $media->id }}" 
                     onclick="handleCardClick(event, '{{ $url }}', '{{ $media->file_name }}')">
                    
                    {{-- Selection Overlay --}}
                    <div class="selection-overlay absolute inset-0 bg-primary/20 border-4 border-primary z-20 hidden pointer-events-none">
                        <div class="absolute top-2 right-2 bg-black text-white p-1">
                            <span class="material-symbols-outlined" style="font-size: 20px;">check_circle</span>
                        </div>
                    </div>

                    {{-- Card Header / Preview --}}
                    <div class="aspect-square bg-gray-50 border-b-2 border-black relative overflow-hidden flex items-center justify-center">
                        @if($isImage)
                            <img src="{{ $url }}" alt="{{ $media->file_name }}" class="w-full h-full object-cover">
                        @else
                            <div class="flex flex-col items-center gap-2 text-text-muted">
                                <span class="material-symbols-outlined" style="font-size: 48px;">
                                    @if(str_contains($media->mime_type, 'video')) movie @elseif(str_contains($media->mime_type, 'pdf')) description @else insert_drive_file @endif
                                </span>
                                <span class="text-[10px] uppercase font-bold mono-text">{{ explode('/', $media->mime_type)[1] ?? 'FILE' }}</span>
                            </div>
                        @endif

                        <div class="absolute top-2 left-2 flex gap-1">
                             <span class="px-2 py-0.5 bg-black text-white text-[9px] font-bold uppercase tracking-tighter">
                                {{ $media->category }}
                            </span>
                        </div>
                    </div>

                    {{-- Card Content --}}
                    <div class="p-4 flex-1 flex flex-col justify-between space-y-3">
                        <div>
                            <h4 class="text-[13px] font-bold text-text-main truncate mb-1" title="{{ $media->file_name }}">
                                {{ $media->file_name }}
                            </h4>
                            <div class="flex items-center justify-between text-[10px] text-text-muted mono-text font-medium">
                                <span>{{ $size }}</span>
                                <span>{{ $media->created_at->format('d/m/y') }}</span>
                            </div>
                        </div>

                        <div class="flex items-center justify-between pt-2 border-t border-black/5">
                            <button onclick="copyToClipboard(event, '{{ $url }}')" class="hover:text-primary transition-colors flex items-center gap-1 group/copy">
                                <span class="material-symbols-outlined text-[18px]">content_copy</span>
                                <span class="text-[10px] font-bold uppercase hidden group-hover/copy:inline">Copy Link</span>
                            </button>
                            
                            <form action="{{ route('admin.media.destroy', $media->id) }}" method="POST" onsubmit="return confirm('Hapus file ini permanen?')">
                                @csrf
                                @method('DELETE')
                                <button type="submit" class="text-text-muted hover:text-red-500 transition-colors">
                                    <span class="material-symbols-outlined text-[18px]">delete</span>
                                </button>
                            </form>
                        </div>
                    </div>
                </div>
            @empty
                <div class="col-span-full py-20 bg-white border-2 border-black shadow-drag">
                    @include('admin.partials.empty-state', [
                        'title'   => 'Media Kosong',
                        'message' => 'Belum ada file yang diunggah atau tidak ditemukan.',
                        'icon'    => 'folder_off'
                    ])
                </div>
            @endforelse
        </div>

        {{-- Pagination --}}
        @if($medias->hasPages())
        <div class="pt-10 flex justify-center">
            <div class="bg-white border-2 border-black p-4 shadow-drag">
                {{ $medias->links() }}
            </div>
        </div>
        @endif

    </div>
</div>

{{-- Bulk Action Bar --}}
<div id="bulk-action-bar" class="fixed bottom-10 left-1/2 -translate-x-1/2 z-50 hidden translate-y-20 opacity-0 transition-all duration-300">
    <div class="bg-black text-white px-8 py-4 border-2 border-white shadow-drag flex items-center gap-8">
        <div class="flex flex-col">
            <span class="text-[10px] font-bold uppercase text-gray-400">Terpilih</span>
            <span id="selected-count" class="text-xl font-black mono-text leading-none">0</span>
        </div>
        <div class="h-8 w-[2px] bg-gray-700"></div>
        <div class="flex gap-4">
            <button onclick="cancelSelection()" class="text-sm font-bold hover:text-primary transition-colors uppercase">Batal</button>
            <form id="bulk-delete-form" action="{{ route('admin.media.bulk-destroy') }}" method="POST" onsubmit="return confirm('Hapus semua file terpilih secara permanen?')">
                @csrf
                @method('DELETE')
                <input type="hidden" name="ids[]" id="bulk-ids">
                <button type="submit" class="bg-red-500 hover:bg-red-600 text-white px-6 py-2 border-2 border-black font-bold text-sm uppercase shadow-[2px_2px_0px_0px_rgba(255,255,255,0.3)] active:translate-x-[1px] active:translate-y-[1px] active:shadow-none transition-all">
                    Hapus Massal
                </button>
            </form>
        </div>
    </div>
</div>

{{-- Upload Modal --}}
<div id="upload-modal" class="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm hidden">
    <div class="bg-surface border-4 border-black w-full max-w-lg shadow-drag animate-in fade-in zoom-in duration-200">
        <div class="bg-black p-4 flex justify-between items-center text-white">
            <h3 class="font-black uppercase tracking-widest flex items-center gap-2">
                <span class="material-symbols-outlined">cloud_upload</span>
                Unggah Media Baru
            </h3>
            <button onclick="closeUploadModal()" class="hover:text-primary transition-colors">
                <span class="material-symbols-outlined">close</span>
            </button>
        </div>
        
        <form action="{{ route('admin.media.store') }}" method="POST" enctype="multipart/form-data" class="p-8 space-y-6">
            @csrf
            <div class="space-y-4">
                <div class="space-y-2">
                    <label class="block text-xs font-black uppercase tracking-wider text-text-main">Pilih File</label>
                    <div class="relative group">
                        <input type="file" name="files[]" multiple required class="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-10" onchange="updateFileLabel(this)">
                        <div class="border-2 border-dashed border-black p-8 text-center group-hover:bg-gray-50 transition-colors">
                            <span class="material-symbols-outlined text-4xl mb-2">upload_file</span>
                            <p class="text-sm font-bold uppercase" id="file-label">Klik atau seret file ke sini</p>
                            <p class="text-[10px] text-text-muted mt-1 uppercase">Maks 10MB per file</p>
                        </div>
                    </div>
                </div>

                <div class="space-y-2">
                    <label class="block text-xs font-black uppercase tracking-wider text-text-main">Kategori Penyimpanan</label>
                    <select name="category" required class="w-full bg-white border-2 border-black p-4 font-bold text-sm focus:ring-primary focus:border-primary uppercase">
                        <option value="gallery">Gallery (Umum/Aset Visual)</option>
                        <option value="materials">Materials (Dokumen/Materi)</option>
                        <option value="attachments">Attachments (Lampiran Lainnya)</option>
                        <option value="avatars">Avatars (Foto Profil)</option>
                    </select>
                </div>
            </div>

            <div class="pt-4 flex gap-3">
                <button type="button" onclick="closeUploadModal()" class="flex-1 bg-white border-2 border-black py-4 font-black uppercase hover:bg-gray-50 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none">
                    Batal
                </button>
                <button type="submit" class="flex-1 bg-primary border-2 border-black py-4 font-black uppercase hover:bg-primary-dark transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none">
                    Mulai Unggah
                </button>
            </div>
        </form>
    </div>
</div>

@endsection

@push('scripts')
<script>
    let isSelectionMode = false;
    let selectedIds = new Set();

    // Toggle Selection Mode
    const toggleBtn = document.getElementById('toggle-selection');
    const selectionIcon = document.getElementById('selection-icon');
    const selectionText = document.getElementById('selection-text');
    const bulkBar = document.getElementById('bulk-action-bar');
    const selectedCountDisplay = document.getElementById('selected-count');

    toggleBtn.addEventListener('click', () => {
        isSelectionMode = !isSelectionMode;
        
        if (isSelectionMode) {
            toggleBtn.classList.replace('bg-white', 'bg-black');
            toggleBtn.classList.replace('text-black', 'text-white');
            selectionIcon.innerText = 'close';
            selectionText.innerText = 'BATAL PILIH';
        } else {
            cancelSelection();
        }
    });

    function handleCardClick(event, url, fileName) {
        // Don't trigger if clicked on child buttons
        if (event.target.closest('button') || event.target.closest('form')) {
            return;
        }

        if (isSelectionMode) {
            const card = event.currentTarget;
            const id = card.dataset.id;
            const overlay = card.querySelector('.selection-overlay');

            if (selectedIds.has(id)) {
                selectedIds.delete(id);
                overlay.classList.add('hidden');
            } else {
                selectedIds.add(id);
                overlay.classList.remove('hidden');
            }

            updateBulkBar();
        } else {
            // Optional: Open detailed view/preview
            window.open(url, '_blank');
        }
    }

    function updateBulkBar() {
        if (selectedIds.size > 0) {
            bulkBar.classList.remove('hidden');
            setTimeout(() => {
                bulkBar.classList.remove('translate-y-20', 'opacity-0');
            }, 10);
            selectedCountDisplay.innerText = selectedIds.size;
            
            // Update hidden input for form
            const bulkForm = document.getElementById('bulk-delete-form');
            // Remove old hidden inputs
            bulkForm.querySelectorAll('input[name="ids[]"]').forEach(i => i.remove());
            // Add new ones
            selectedIds.forEach(id => {
                const input = document.createElement('input');
                input.type = 'hidden';
                input.name = 'ids[]';
                input.value = id;
                bulkForm.appendChild(input);
            });
        } else {
            bulkBar.classList.add('translate-y-20', 'opacity-0');
            setTimeout(() => {
                if (selectedIds.size === 0) bulkBar.classList.add('hidden');
            }, 300);
        }
    }

    function cancelSelection() {
        isSelectionMode = false;
        selectedIds.clear();
        
        // UI Reset
        toggleBtn.classList.replace('bg-black', 'bg-white');
        toggleBtn.classList.replace('text-white', 'text-black');
        selectionIcon.innerText = 'checklist';
        selectionText.innerText = 'PILIH MASSAL';
        
        document.querySelectorAll('.selection-overlay').forEach(el => el.classList.add('hidden'));
        updateBulkBar();
    }

    // Modal Controls
    function openUploadModal() {
        document.getElementById('upload-modal').classList.remove('hidden');
    }

    function closeUploadModal() {
        document.getElementById('upload-modal').classList.add('hidden');
    }

    function updateFileLabel(input) {
        const label = document.getElementById('file-label');
        if (input.files.length > 0) {
            label.innerText = `${input.files.length} file dipilih`;
            label.classList.add('text-primary');
        } else {
            label.innerText = 'Klik atau seret file ke sini';
            label.classList.remove('text-primary');
        }
    }

    // Clipboard Functionality
    async function copyToClipboard(event, text) {
        event.stopPropagation();
        const btn = event.currentTarget;
        const originalContent = btn.innerHTML;

        try {
            await navigator.clipboard.writeText(text);
            btn.innerHTML = `
                <span class="material-symbols-outlined text-[18px] text-green-500">check_circle</span>
                <span class="text-[10px] font-bold uppercase text-green-500">Berhasil!</span>
            `;
            setTimeout(() => {
                btn.innerHTML = originalContent;
            }, 2000);
        } catch (err) {
            console.error('Failed to copy: ', err);
        }
    }
</script>
@endpush
