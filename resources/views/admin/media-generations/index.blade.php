@extends('admin.layouts.app')

@section('title', 'Media Generation Debug')
@section('page-title', 'Media Generation Debug')
@section('page-description', 'Tinjau inferensi taxonomy dan hint drafting tanpa membuka payload API secara manual.')

@php
    $statusClasses = [
        'queued' => 'bg-slate-100 text-slate-700 border-black',
        'interpreting' => 'bg-amber-100 text-amber-800 border-black',
        'classified' => 'bg-yellow-100 text-yellow-800 border-black',
        'generating' => 'bg-sky-100 text-sky-800 border-black',
        'uploading' => 'bg-cyan-100 text-cyan-800 border-black',
        'publishing' => 'bg-indigo-100 text-indigo-800 border-black',
        'completed' => 'bg-emerald-100 text-emerald-800 border-black',
        'failed' => 'bg-red-100 text-red-800 border-black',
        'cancelled' => 'bg-zinc-100 text-zinc-700 border-black',
    ];
@endphp

@section('content')
<div class="h-full overflow-y-auto p-8 scrollbar-thin">
    <div class="mx-auto max-w-[1600px] space-y-8 pb-24">
        <div class="grid grid-cols-1 gap-6 xl:grid-cols-[minmax(0,520px)_minmax(0,1fr)]">
            <div class="space-y-6">
                <div class="bg-surface border-2 border-black p-6 shadow-drag">
                    <div class="mb-5 flex items-start justify-between gap-4">
                        <div>
                            <h2 class="text-xl font-black uppercase tracking-tight text-text-main">Antrian Generation</h2>
                            <p class="mt-1 text-sm text-text-muted">Pilih satu generation untuk memuat panel taxonomy debug dari endpoint admin API.</p>
                        </div>
                        <div class="bg-black px-3 py-2 text-right text-white">
                            <div class="text-[10px] uppercase tracking-[0.2em] text-gray-300">Halaman Ini</div>
                            <div class="font-mono text-lg font-black">{{ $mediaGenerations->count() }}</div>
                        </div>
                    </div>

                    <form method="GET" action="{{ route('admin.media-generations.index') }}" class="grid grid-cols-1 gap-4 md:grid-cols-[minmax(0,1fr)_180px_auto]">
                        <div class="relative">
                            <div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-4 text-text-muted">
                                <span class="material-symbols-outlined" style="font-size: 20px;">search</span>
                            </div>
                            <input type="text" name="search" value="{{ $search }}" placeholder="Cari prompt, UUID, guru..." class="block w-full border-2 border-black bg-white p-3 pl-12 text-[14px] font-medium text-text-main focus:border-primary focus:ring-primary">
                        </div>

                        <div>
                            <select name="status" class="block w-full border-2 border-black bg-white p-3 text-[14px] font-bold uppercase text-text-main focus:border-primary focus:ring-primary">
                                <option value="">Semua Status</option>
                                @foreach($statuses as $statusOption)
                                    <option value="{{ $statusOption }}" {{ $status === $statusOption ? 'selected' : '' }}>{{ strtoupper($statusOption) }}</option>
                                @endforeach
                            </select>
                        </div>

                        <div class="flex gap-2">
                            <button type="submit" class="bg-black px-5 py-3 font-bold uppercase text-white transition hover:bg-gray-800">Filter</button>
                            @if(request()->anyFilled(['search', 'status', 'generation']))
                                <a href="{{ route('admin.media-generations.index') }}" class="flex items-center justify-center border-2 border-black bg-white px-4 font-bold transition hover:bg-gray-50">
                                    <span class="material-symbols-outlined">restart_alt</span>
                                </a>
                            @endif
                        </div>
                    </form>
                </div>

                <div class="overflow-hidden border-2 border-black bg-white shadow-drag">
                    @if($mediaGenerations->isEmpty())
                        <div class="px-8 py-20">
                            @include('admin.partials.empty-state', [
                                'title' => 'Belum Ada Media Generation',
                                'message' => 'Tidak ada data yang cocok dengan filter saat ini.',
                                'icon' => 'analytics_off',
                            ])
                        </div>
                    @else
                        <div class="border-b-2 border-black bg-black px-5 py-4 text-white">
                            <div class="grid grid-cols-[minmax(0,1fr)_110px] gap-4 text-[11px] font-black uppercase tracking-[0.2em] text-gray-300">
                                <div>Prompt / Guru</div>
                                <div>Status</div>
                            </div>
                        </div>

                        <div class="divide-y-2 divide-black/10">
                            @foreach($mediaGenerations as $generation)
                                @php
                                    $subject = $generation->subSubject?->subject ?? $generation->subject;
                                    $teacherLabel = trim((string) data_get($generation, 'teacher.name', 'Guru tidak diketahui'));
                                    $taxonomyLabel = trim(implode(' / ', array_filter([
                                        $subject?->name,
                                        $generation->subSubject?->name,
                                    ], fn ($value) => is_string($value) && trim($value) !== '')));
                                @endphp
                                <button
                                    type="button"
                                    class="generation-row block w-full px-5 py-4 text-left transition hover:bg-gray-50"
                                    data-generation-id="{{ $generation->id }}"
                                    data-debug-url="{{ url('/api/v1/admin/media-generations/' . $generation->id . '/debug-taxonomy') }}"
                                    data-poll-url="{{ url('/api/v1/media-generations/' . $generation->id) }}"
                                >
                                    <div class="grid grid-cols-[minmax(0,1fr)_110px] gap-4">
                                        <div class="space-y-3">
                                            <div class="flex items-start justify-between gap-3">
                                                <div>
                                                    <div class="font-mono text-[11px] font-bold uppercase tracking-[0.18em] text-text-muted">{{ $generation->id }}</div>
                                                    <h3 class="mt-1 text-sm font-bold text-text-main">{{ \Illuminate\Support\Str::limit((string) $generation->raw_prompt, 92) }}</h3>
                                                </div>
                                                <span class="inline-flex border-2 px-2 py-1 text-[10px] font-black uppercase tracking-[0.15em] {{ $statusClasses[$generation->status] ?? 'bg-white text-text-main border-black' }}">
                                                    {{ strtoupper((string) $generation->status) }}
                                                </span>
                                            </div>

                                            <div class="flex flex-wrap gap-x-4 gap-y-2 text-[11px] text-text-muted">
                                                <span><span class="font-bold uppercase text-text-main">Guru:</span> {{ $teacherLabel }}</span>
                                                @if($taxonomyLabel !== '')
                                                    <span><span class="font-bold uppercase text-text-main">Taxonomy:</span> {{ $taxonomyLabel }}</span>
                                                @endif
                                                <span><span class="font-bold uppercase text-text-main">Update:</span> {{ optional($generation->updated_at)->format('d M Y H:i') }}</span>
                                            </div>
                                        </div>

                                        <div class="flex items-center justify-end">
                                            <span class="inline-flex items-center gap-1 border-2 border-black bg-white px-3 py-2 text-[11px] font-black uppercase tracking-[0.15em] text-text-main">
                                                <span class="material-symbols-outlined" style="font-size: 16px;">manage_search</span>
                                                Inspect
                                            </span>
                                        </div>
                                    </div>
                                </button>
                            @endforeach
                        </div>
                    @endif
                </div>

                @if($mediaGenerations->hasPages())
                    <div class="flex justify-center pt-2">
                        <div class="border-2 border-black bg-white p-4 shadow-drag">
                            {{ $mediaGenerations->links() }}
                        </div>
                    </div>
                @endif
            </div>

            <div class="min-h-[720px] border-2 border-black bg-white shadow-drag">
                <div class="border-b-2 border-black bg-black px-6 py-4 text-white">
                    <div class="flex items-start justify-between gap-4">
                        <div>
                            <h2 class="text-xl font-black uppercase tracking-tight">Panel Taxonomy Debug</h2>
                            <p class="mt-1 text-sm text-gray-300">Panel ini memuat data langsung dari endpoint admin API untuk generation yang dipilih.</p>
                        </div>
                        <div id="debug-panel-badge" class="hidden border-2 border-white px-3 py-2 text-[10px] font-black uppercase tracking-[0.18em] text-white"></div>
                    </div>
                </div>

                <div id="generation-debug-panel" class="h-full p-6">
                    <div class="flex h-full min-h-[620px] items-center justify-center border-2 border-dashed border-black/20 bg-gray-50 p-10 text-center">
                        <div class="max-w-md space-y-3">
                            <span class="material-symbols-outlined text-[56px] text-text-muted">data_object</span>
                            <h3 class="text-lg font-black uppercase text-text-main">Pilih Generation</h3>
                            <p class="text-sm text-text-muted">Klik salah satu item di daftar kiri untuk memuat inferensi taxonomy, hint drafting, dan kandidat subject yang dipakai sistem.</p>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
</div>
@endsection

@push('scripts')
<script>
    const debugPanel = document.getElementById('generation-debug-panel');
    const debugPanelBadge = document.getElementById('debug-panel-badge');
    const generationRows = Array.from(document.querySelectorAll('.generation-row'));
    const initialGenerationId = @json($selectedGenerationId);

    function escapeHtml(value) {
        return String(value ?? '')
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#039;');
    }

    function normalizeText(value, fallback = '-') {
        const normalized = String(value ?? '').trim();
        return normalized !== '' ? normalized : fallback;
    }

    function renderStatusChip(status) {
        const normalizedStatus = String(status ?? '').trim().toLowerCase();
        const classMap = {
            queued: 'bg-slate-100 text-slate-700 border-black',
            interpreting: 'bg-amber-100 text-amber-800 border-black',
            classified: 'bg-yellow-100 text-yellow-800 border-black',
            generating: 'bg-sky-100 text-sky-800 border-black',
            uploading: 'bg-cyan-100 text-cyan-800 border-black',
            publishing: 'bg-indigo-100 text-indigo-800 border-black',
            completed: 'bg-emerald-100 text-emerald-800 border-black',
            failed: 'bg-red-100 text-red-800 border-black',
            cancelled: 'bg-zinc-100 text-zinc-700 border-black',
        };

        const classes = classMap[normalizedStatus] ?? 'bg-white text-text-main border-black';

        return `<span class="inline-flex border-2 px-3 py-1 text-[11px] font-black uppercase tracking-[0.16em] ${classes}">${escapeHtml(normalizedStatus || 'unknown')}</span>`;
    }

    function renderKeyValueCard(label, value, extra = '') {
        return `
            <div class="border-2 border-black bg-gray-50 p-4 shadow-drag">
                <div class="text-[10px] font-black uppercase tracking-[0.18em] text-text-muted">${escapeHtml(label)}</div>
                <div class="mt-2 text-sm font-bold text-text-main">${escapeHtml(normalizeText(value))}</div>
                ${extra}
            </div>
        `;
    }

    function renderList(items, emptyLabel = 'Tidak ada data.') {
        if (!Array.isArray(items) || items.length === 0) {
            return `<div class="text-sm text-text-muted">${escapeHtml(emptyLabel)}</div>`;
        }

        return `
            <ul class="space-y-2">
                ${items.map((item) => `
                    <li class="flex items-start gap-2 border-2 border-black/10 bg-white px-3 py-2 text-sm text-text-main">
                        <span class="material-symbols-outlined text-[16px] text-text-muted">subdirectory_arrow_right</span>
                        <span>${escapeHtml(item)}</span>
                    </li>
                `).join('')}
            </ul>
        `;
    }

    function renderCandidateMatches(matches) {
        if (!Array.isArray(matches) || matches.length === 0) {
            return '<div class="text-sm text-text-muted">Tidak ada kandidat tambahan.</div>';
        }

        return `
            <div class="overflow-x-auto border-2 border-black">
                <table class="min-w-full divide-y-2 divide-black/10 bg-white text-left text-sm">
                    <thead class="bg-black text-white">
                        <tr>
                            <th class="px-4 py-3 text-[10px] font-black uppercase tracking-[0.18em]">Subject</th>
                            <th class="px-4 py-3 text-[10px] font-black uppercase tracking-[0.18em]">Sub-Subject</th>
                            <th class="px-4 py-3 text-[10px] font-black uppercase tracking-[0.18em]">Jenjang</th>
                            <th class="px-4 py-3 text-[10px] font-black uppercase tracking-[0.18em]">Kelas</th>
                            <th class="px-4 py-3 text-[10px] font-black uppercase tracking-[0.18em]">Confidence</th>
                        </tr>
                    </thead>
                    <tbody>
                        ${matches.map((item) => `
                            <tr class="border-t border-black/10">
                                <td class="px-4 py-3 font-semibold text-text-main">${escapeHtml(normalizeText(item.subject_name))}</td>
                                <td class="px-4 py-3 text-text-main">${escapeHtml(normalizeText(item.sub_subject_name))}</td>
                                <td class="px-4 py-3 text-text-muted">${escapeHtml(normalizeText(item.jenjang))}</td>
                                <td class="px-4 py-3 text-text-muted">${escapeHtml(normalizeText(item.kelas))}</td>
                                <td class="px-4 py-3 text-text-main">${escapeHtml(normalizeText(item.label))} (${escapeHtml(normalizeText(item.score))})</td>
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            </div>
        `;
    }

    function renderContextTable(title, context) {
        const entries = Object.entries(context || {}).filter(([, value]) => String(value ?? '').trim() !== '');

        return `
            <div class="border-2 border-black bg-white p-5 shadow-drag">
                <h3 class="text-sm font-black uppercase tracking-[0.18em] text-text-main">${escapeHtml(title)}</h3>
                <div class="mt-4 space-y-3 text-sm">
                    ${entries.length === 0 ? '<div class="text-text-muted">Tidak ada data.</div>' : entries.map(([key, value]) => `
                        <div class="grid grid-cols-[140px_minmax(0,1fr)] gap-3 border-b border-black/10 pb-3 last:border-b-0 last:pb-0">
                            <div class="font-black uppercase tracking-[0.14em] text-text-muted">${escapeHtml(key.replaceAll('_', ' '))}</div>
                            <div class="font-medium text-text-main">${escapeHtml(normalizeText(value))}</div>
                        </div>
                    `).join('')}
                </div>
            </div>
        `;
    }

    function renderDebugPayload(payload) {
        const bestMatch = payload.taxonomy_inference?.best_match ?? {};
        const confidence = payload.taxonomy_inference?.confidence ?? {};
        const draftHint = payload.draft_taxonomy_hint ?? {};
        const draftGuidance = draftHint.content_guidance ?? {};
        const gradeContext = draftHint.grade_context ?? {};

        debugPanelBadge.textContent = `GEN ${payload.id}`;
        debugPanelBadge.classList.remove('hidden');

        debugPanel.innerHTML = `
            <div class="space-y-6">
                <div class="border-2 border-black bg-[#FFF9E8] p-6 shadow-drag">
                    <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                        <div class="space-y-3">
                            <div class="font-mono text-[11px] font-black uppercase tracking-[0.18em] text-text-muted">${escapeHtml(payload.id)}</div>
                            <h2 class="text-xl font-black text-text-main">${escapeHtml(normalizeText(payload.prompt, 'Prompt tidak tersedia'))}</h2>
                            <div class="flex flex-wrap gap-3">
                                ${renderStatusChip(payload.status)}
                                <a href="${escapeHtml(payload.links?.poll ?? '#')}" target="_blank" rel="noreferrer" class="inline-flex items-center gap-2 border-2 border-black bg-white px-3 py-1 text-[11px] font-black uppercase tracking-[0.16em] text-text-main transition hover:bg-gray-50">
                                    <span class="material-symbols-outlined" style="font-size: 16px;">open_in_new</span>
                                    Poll Resource
                                </a>
                            </div>
                        </div>
                        <div class="min-w-[220px] border-2 border-black bg-white p-4 text-sm shadow-drag">
                            <div class="text-[10px] font-black uppercase tracking-[0.18em] text-text-muted">Drafting</div>
                            <div class="mt-2 text-base font-black text-text-main">${escapeHtml(normalizeText(payload.drafting?.source))}</div>
                            <div class="mt-2 text-text-muted">Fallback: ${escapeHtml(payload.drafting?.fallback_triggered ? 'true' : 'false')}</div>
                            <div class="text-text-muted">Reason: ${escapeHtml(normalizeText(payload.drafting?.fallback_reason_code))}</div>
                        </div>
                    </div>
                </div>

                <div class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
                    ${renderKeyValueCard('Persisted Subject', payload.persisted_taxonomy?.subject?.name)}
                    ${renderKeyValueCard('Persisted Sub-Subject', payload.persisted_taxonomy?.sub_subject?.name)}
                    ${renderKeyValueCard('Inference Confidence', `${normalizeText(confidence.label)} (${normalizeText(confidence.score)})`)}
                    ${renderKeyValueCard('Draft Hint Source', draftHint.source)}
                </div>

                <div class="grid grid-cols-1 gap-6 xl:grid-cols-[minmax(0,1fr)_340px]">
                    <div class="space-y-6">
                        <div class="border-2 border-black bg-white p-5 shadow-drag">
                            <h3 class="text-sm font-black uppercase tracking-[0.18em] text-text-main">Best Match</h3>
                            <div class="mt-4 grid grid-cols-1 gap-4 md:grid-cols-2">
                                ${renderKeyValueCard('Subject', bestMatch.subject_name)}
                                ${renderKeyValueCard('Sub-Subject', bestMatch.sub_subject_name)}
                                ${renderKeyValueCard('Jenjang', bestMatch.jenjang)}
                                ${renderKeyValueCard('Kelas / Semester / Bab', `${normalizeText(bestMatch.kelas)} / ${normalizeText(bestMatch.semester)} / ${normalizeText(bestMatch.bab)}`)}
                            </div>
                            <div class="mt-5 space-y-4">
                                <div>
                                    <div class="text-[10px] font-black uppercase tracking-[0.18em] text-text-muted">Description</div>
                                    <p class="mt-2 text-sm leading-6 text-text-main">${escapeHtml(normalizeText(bestMatch.description))}</p>
                                </div>
                                <div>
                                    <div class="text-[10px] font-black uppercase tracking-[0.18em] text-text-muted">Content Structure</div>
                                    <p class="mt-2 text-sm leading-6 text-text-main">${escapeHtml(normalizeText(bestMatch.content_structure))}</p>
                                </div>
                            </div>
                        </div>

                        <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
                            <div class="border-2 border-black bg-white p-5 shadow-drag">
                                <h3 class="text-sm font-black uppercase tracking-[0.18em] text-text-main">Draft Structure Items</h3>
                                <div class="mt-4">${renderList(draftGuidance.structure_items, 'Tidak ada structure items.')}</div>
                            </div>

                            <div class="border-2 border-black bg-white p-5 shadow-drag">
                                <h3 class="text-sm font-black uppercase tracking-[0.18em] text-text-main">Matched Signals</h3>
                                <div class="mt-4">${renderList(draftHint.matched_signals ?? bestMatch.matched_signals ?? [], 'Tidak ada matched signal.')}</div>
                            </div>
                        </div>

                        <div class="border-2 border-black bg-white p-5 shadow-drag">
                            <h3 class="text-sm font-black uppercase tracking-[0.18em] text-text-main">Candidate Matches</h3>
                            <div class="mt-4">${renderCandidateMatches(payload.taxonomy_inference?.candidate_matches)}</div>
                        </div>

                        <div class="border-2 border-black bg-white p-5 shadow-drag">
                            <h3 class="text-sm font-black uppercase tracking-[0.18em] text-text-main">Raw JSON</h3>
                            <details class="mt-4 border-2 border-black bg-gray-50 p-4">
                                <summary class="cursor-pointer text-sm font-black uppercase tracking-[0.16em] text-text-main">Tampilkan Payload</summary>
                                <pre class="mt-4 overflow-x-auto bg-black p-4 font-mono text-[12px] leading-6 text-green-300">${escapeHtml(JSON.stringify(payload, null, 2))}</pre>
                            </details>
                        </div>
                    </div>

                    <div class="space-y-6">
                        ${renderContextTable('Draft Grade Context', gradeContext)}
                        ${renderContextTable('Interpretation Subject Context', payload.interpretation_context?.subject_context ?? {})}
                        ${renderContextTable('Interpretation Sub-Subject Context', payload.interpretation_context?.sub_subject_context ?? {})}

                        <div class="border-2 border-black bg-white p-5 shadow-drag">
                            <h3 class="text-sm font-black uppercase tracking-[0.18em] text-text-main">Draft Content Guidance</h3>
                            <div class="mt-4 space-y-4 text-sm text-text-main">
                                <div>
                                    <div class="text-[10px] font-black uppercase tracking-[0.18em] text-text-muted">Description</div>
                                    <p class="mt-2 leading-6">${escapeHtml(normalizeText(draftGuidance.description))}</p>
                                </div>
                                <div>
                                    <div class="text-[10px] font-black uppercase tracking-[0.18em] text-text-muted">Structure</div>
                                    <p class="mt-2 leading-6">${escapeHtml(normalizeText(draftGuidance.structure))}</p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        `;
    }

    function setActiveRow(activeRow) {
        generationRows.forEach((row) => {
            const isActive = row === activeRow;
            row.classList.toggle('bg-primary/10', isActive);
            row.classList.toggle('ring-2', isActive);
            row.classList.toggle('ring-black', isActive);
        });
    }

    function setPanelLoading(generationId) {
        debugPanelBadge.classList.remove('hidden');
        debugPanelBadge.textContent = `GEN ${generationId}`;
        debugPanel.innerHTML = `
            <div class="flex min-h-[620px] items-center justify-center border-2 border-dashed border-black/20 bg-gray-50 p-10 text-center">
                <div class="max-w-md space-y-3">
                    <span class="material-symbols-outlined animate-pulse text-[56px] text-text-muted">hourglass_top</span>
                    <h3 class="text-lg font-black uppercase text-text-main">Memuat Debug Payload</h3>
                    <p class="text-sm text-text-muted">Sedang mengambil data taxonomy dari endpoint admin API.</p>
                </div>
            </div>
        `;
    }

    function setPanelError(message) {
        debugPanel.innerHTML = `
            <div class="flex min-h-[620px] items-center justify-center border-2 border-black bg-red-50 p-10 text-center">
                <div class="max-w-lg space-y-3">
                    <span class="material-symbols-outlined text-[56px] text-red-600">error</span>
                    <h3 class="text-lg font-black uppercase text-text-main">Gagal Memuat Debug Payload</h3>
                    <p class="text-sm leading-6 text-text-muted">${escapeHtml(message)}</p>
                </div>
            </div>
        `;
    }

    async function loadDebugForRow(row) {
        const generationId = row.dataset.generationId;
        const debugUrl = row.dataset.debugUrl;

        if (!generationId || !debugUrl) {
            return;
        }

        setActiveRow(row);
        setPanelLoading(generationId);

        const nextUrl = new URL(window.location.href);
        nextUrl.searchParams.set('generation', generationId);
        window.history.replaceState({}, '', nextUrl);

        try {
            const response = await fetch(debugUrl, {
                method: 'GET',
                credentials: 'same-origin',
                headers: {
                    'Accept': 'application/json',
                    'X-Requested-With': 'XMLHttpRequest',
                },
            });

            const payload = await response.json();

            if (!response.ok || !payload?.data) {
                throw new Error(payload?.message || 'Endpoint admin API mengembalikan respons yang tidak dapat dipakai.');
            }

            renderDebugPayload(payload.data);
        } catch (error) {
            setPanelError(error instanceof Error ? error.message : 'Terjadi kegagalan tak dikenal saat memuat debug payload.');
        }
    }

    generationRows.forEach((row) => {
        row.addEventListener('click', () => loadDebugForRow(row));
    });

    if (generationRows.length > 0) {
        const initialRow = generationRows.find((row) => row.dataset.generationId === initialGenerationId) || generationRows[0];

        if (initialRow) {
            loadDebugForRow(initialRow);
        }
    }
</script>
@endpush