@extends('admin.layouts.app')

@section('title', 'Activity Logs')
@section('page-title', 'Activity Logs')
@section('page-description', 'Pantau aktivitas penting yang dilakukan oleh admin atau sistem.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1200px] mx-auto space-y-8 pb-10">

        {{-- Filter & Search --}}
        <div class="bg-surface border-2 border-black p-6 shadow-drag">
            <form method="GET" action="{{ route('admin.activity-logs.index') }}" class="space-y-4">
                <div class="flex flex-col md:flex-row gap-4">
                    {{-- Search Bar --}}
                    <div class="flex-1 relative">
                        <div class="absolute inset-y-0 left-0 flex items-center pl-4 pointer-events-none text-text-muted">
                            <span class="material-symbols-outlined" style="font-size: 20px;">search</span>
                        </div>
                        <input type="text" name="search" value="{{ $search }}" placeholder="Cari actor, subject ID, atau action..." class="bg-white border-2 border-black text-text-main text-[14px] font-medium focus:ring-primary focus:border-primary block w-full pl-12 p-3">
                    </div>
                </div>

                <div class="flex flex-wrap gap-4 pt-2">
                    {{-- Action Filter --}}
                    <div class="min-w-[200px]">
                        <label class="block text-[11px] font-bold uppercase mono-text mb-1.5 ml-1">Action</label>
                        <select name="action" class="bg-white border-2 border-black text-text-main text-[13px] font-bold focus:ring-primary focus:border-primary block w-full p-2.5">
                            <option value="">SEMUA ACTION</option>
                            @foreach($actions as $act)
                                <option value="{{ $act }}" @selected($action == $act)>{{ strtoupper($act) }}</option>
                            @endforeach
                        </select>
                    </div>

                    {{-- Actor Filter --}}
                    <div class="min-w-[200px]">
                        <label class="block text-[11px] font-bold uppercase mono-text mb-1.5 ml-1">Actor</label>
                        <select name="actor_id" class="bg-white border-2 border-black text-text-main text-[13px] font-bold focus:ring-primary focus:border-primary block w-full p-2.5">
                            <option value="">SEMUA ACTOR</option>
                            @foreach($actors as $actor)
                                <option value="{{ $actor->id }}" @selected($actorId == $actor->id)>{{ strtoupper($actor->name) }}</option>
                            @endforeach
                        </select>
                    </div>

                    {{-- Entity Filter --}}
                    <div class="min-w-[200px]">
                        <label class="block text-[11px] font-bold uppercase mono-text mb-1.5 ml-1">Entity</label>
                        <select name="subject_type" class="bg-white border-2 border-black text-text-main text-[13px] font-bold focus:ring-primary focus:border-primary block w-full p-2.5">
                            <option value="">SEMUA ENTITY</option>
                            @foreach($entityTypes as $type)
                                <option value="{{ $type }}" @selected($entityType == $type)>{{ strtoupper(class_basename($type)) }}</option>
                            @endforeach
                        </select>
                    </div>

                    <div class="flex items-end gap-3 mt-4 md:mt-0">
                        <button type="submit" class="bg-black hover:bg-gray-800 text-white font-bold px-8 py-2.5 transition shadow-drag active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase tracking-wider text-[13px]">
                            FILTER
                        </button>
                        @if(request()->anyFilled(['action', 'actor_id', 'subject_type', 'search', 'date_from', 'date_to']))
                            <a href="{{ route('admin.activity-logs.index') }}" class="flex items-center justify-center bg-white border-2 border-black px-6 py-2.5 font-bold hover:bg-gray-50 transition uppercase tracking-wider text-[13px]">
                                RESET
                            </a>
                        @endif
                    </div>
                </div>
            </form>
        </div>

        {{-- Table Container --}}
        <div class="bg-surface border-2 border-black shadow-drag overflow-hidden">
            <div class="overflow-x-auto">
                <table class="w-full text-left text-[13px]">
                    <thead class="bg-gray-50 border-b-2 border-black">
                        <tr>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Waktu</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Actor</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Action</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider">Entity</th>
                            <th scope="col" class="px-6 py-4 uppercase mono-text font-bold text-text-main tracking-wider text-right">Metadata</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-black/10 text-text-main font-medium">
                        @forelse($logs as $log)
                        <tr class="hover:bg-gray-50 transition-colors group">
                            <td class="px-6 py-5 mono-text text-text-muted">
                                {{ $log->created_at->format('d M Y') }}
                                <div class="text-[10px] mt-0.5 opacity-60">{{ $log->created_at->format('H:i:s') }}</div>
                            </td>
                            <td class="px-6 py-5 border-l border-black/5">
                                @if($log->actor)
                                    <div class="font-bold text-[14px] text-text-main leading-tight">{{ $log->actor->name }}</div>
                                    <div class="text-[11px] mono-text text-text-muted mt-1">{{ $log->actor->email }}</div>
                                @else
                                    <span class="text-text-muted italic mono-text text-[11px]">SYSTEM / DELETED</span>
                                @endif
                            </td>
                            <td class="px-6 py-5">
                                @php
                                    $actionColor = match(strtolower($log->action)) {
                                        'create', 'created', 'store' => 'bg-[#B4E380]',
                                        'update', 'updated', 'edit' => 'bg-[#FFD93D]',
                                        'delete', 'deleted', 'destroy' => 'bg-[#FF6B6B] text-white',
                                        default => 'bg-white',
                                    };
                                @endphp
                                <span class="inline-flex items-center px-2.5 py-0.5 border border-black text-[10px] font-bold uppercase tracking-tighter shadow-[2px_2px_0px_rgba(0,0,0,1)] {{ $actionColor }}">
                                    {{ $log->action }}
                                </span>
                            </td>
                            <td class="px-6 py-5">
                                <div class="font-bold text-text-main uppercase text-[12px]">{{ class_basename($log->subject_type) }}</div>
                                <div class="text-[11px] mono-text text-text-muted mt-1 bg-gray-100 border border-black/10 px-1 inline-block" title="{{ $log->subject_id }}">
                                    ID: {{ Str::limit($log->subject_id, 13) }}
                                </div>
                            </td>
                            <td class="px-6 py-5 text-right">
                                <button onclick="showMetadataModal('{{ $log->id }}', {{ json_encode($log->metadata) }})" class="bg-white hover:bg-primary border-2 border-black text-black font-bold px-4 py-1.5 transition shadow-[3px_3px_0px_rgba(0,0,0,1)] active:translate-x-[1px] active:translate-y-[1px] active:shadow-none text-[11px] uppercase">
                                    DETAIL
                                </button>
                            </td>
                        </tr>
                        @empty
                        <tr>
                            <td colspan="5" class="py-12 bg-white text-center">
                                @include('admin.partials.empty-state', [
                                    'title'   => 'Log tidak ditemukan',
                                    'message' => 'Belum ada activity log yang sesuai dengan filter Anda.',
                                    'icon'    => 'history_toggle_off'
                                ])
                            </td>
                        </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>
            
            {{-- Pagination --}}
            @if($logs->hasPages())
            <div class="px-6 py-6 border-t-2 border-black bg-gray-50">
                {{ $logs->links() }}
            </div>
            @endif
        </div>
    </div>
</div>

{{-- Metadata Modal --}}
<div id="metadata-modal" class="fixed inset-0 z-50 hidden flex items-center justify-center p-4 bg-black/60 backdrop-blur-[2px]">
    <div class="bg-surface border-[3px] border-black shadow-[8px_8px_0px_rgba(0,0,0,1)] w-full max-w-2xl transform transition-all">
        <div class="bg-black text-white p-4 flex justify-between items-center">
            <h3 class="font-bold uppercase tracking-wider mono-text text-[14px]">Log Metadata <span id="log-id-display" class="text-primary ml-2 italic"></span></h3>
            <button onclick="closeMetadataModal()" class="text-white hover:text-primary transition-colors">
                <span class="material-symbols-outlined font-bold">close</span>
            </button>
        </div>
        <div class="p-6">
            <div class="bg-white border-2 border-black p-4 overflow-auto max-h-[60vh]">
                <pre id="metadata-content" class="text-[12px] mono-text text-text-main"></pre>
            </div>
            <div class="mt-6 flex justify-end">
                <button onclick="closeMetadataModal()" class="bg-black hover:bg-gray-800 text-white font-bold px-10 py-3 shadow-drag transition active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase tracking-wider text-[13px]">
                    TUTUP
                </button>
            </div>
        </div>
    </div>
</div>

@push('scripts')
<script>
    function showMetadataModal(id, metadata) {
        const modal = document.getElementById('metadata-modal');
        const idDisplay = document.getElementById('log-id-display');
        const content = document.getElementById('metadata-content');
        
        idDisplay.textContent = '#' + id;
        content.textContent = JSON.stringify(metadata, null, 4);
        
        modal.classList.remove('hidden');
        document.body.style.overflow = 'hidden';
    }

    function closeMetadataModal() {
        const modal = document.getElementById('metadata-modal');
        modal.classList.add('hidden');
        document.body.style.overflow = 'auto';
    }

    // Close modal on escape key
    document.addEventListener('keydown', function(e) {
        if (e.key === 'Escape') closeMetadataModal();
    });

    // Close on click outside
    document.getElementById('metadata-modal').addEventListener('click', function(e) {
        if (e.target === this) closeMetadataModal();
    });
</script>
@endpush
@endsection

