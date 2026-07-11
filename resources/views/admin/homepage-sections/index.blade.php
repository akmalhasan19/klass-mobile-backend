@extends('admin.layouts.app')

@section('title', 'Homepage Configurator')
@section('page-title', 'Homepage Configurator')
@section('page-description', 'Curate the mobile experience: manage sections and recommended projects.')

@section('content')
<div class="w-full p-6 space-y-6">

    <div class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <div class="flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
            <div>
                <h2 class="text-lg font-bold text-gray-900">Section Ordering</h2>
                <p class="text-sm text-gray-500">Update labels, ordering, and visibility for the mobile homepage sections.</p>
            </div>
            <button type="submit" form="sectionOrderingForm" class="inline-flex w-fit items-center rounded-lg bg-slate-900 px-4 py-2 text-sm font-semibold text-white shadow-sm transition hover:bg-slate-800">
                Save Sections
            </button>
        </div>

        <form action="{{ route('admin.homepage-sections.update') }}" method="POST" id="sectionOrderingForm" class="mt-6 overflow-hidden rounded-lg border border-gray-200">
            @csrf
            @method('PATCH')

            <table class="min-w-full divide-y divide-gray-200">
                <thead class="bg-gray-50">
                    <tr>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Section</th>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Position</th>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Data Source</th>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Enabled</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-gray-200 bg-white">
                    @forelse($homepageSections as $index => $section)
                    <tr>
                        <td class="px-6 py-4 align-top">
                            <input type="hidden" name="sections[{{ $index }}][id]" value="{{ $section->id }}">
                            <label class="block text-xs font-semibold uppercase tracking-wide text-gray-500">Label</label>
                            <input
                                type="text"
                                name="sections[{{ $index }}][label]"
                                value="{{ $section->label }}"
                                class="mt-2 w-full rounded-lg border border-gray-300 px-3 py-2 text-sm text-gray-900 focus:border-slate-900 focus:outline-none focus:ring-0"
                            >
                            <p class="mt-2 text-xs text-gray-500">Key: <span class="font-mono">{{ $section->key }}</span></p>
                        </td>
                        <td class="px-6 py-4 align-top">
                            <label class="block text-xs font-semibold uppercase tracking-wide text-gray-500">Position</label>
                            <input
                                type="number"
                                min="1"
                                name="sections[{{ $index }}][position]"
                                value="{{ $section->position }}"
                                class="mt-2 w-28 rounded-lg border border-gray-300 px-3 py-2 text-sm text-gray-900 focus:border-slate-900 focus:outline-none focus:ring-0"
                            >
                        </td>
                        <td class="px-6 py-4 align-top text-sm text-gray-600">
                            <span class="inline-flex rounded-full bg-slate-100 px-3 py-1 font-mono text-xs text-slate-700">{{ $section->data_source }}</span>
                        </td>
                        <td class="px-6 py-4 align-top">
                            <label class="inline-flex items-center gap-3 text-sm text-gray-700">
                                <input
                                    type="checkbox"
                                    name="sections[{{ $index }}][is_enabled]"
                                    value="1"
                                    @checked($section->is_enabled)
                                    class="h-4 w-4 rounded border-gray-300 text-slate-900 focus:ring-0"
                                >
                                Enabled
                            </label>
                        </td>
                    </tr>
                    @empty
                    <tr>
                        <td colspan="4" class="px-6 py-4 text-center text-sm text-gray-500">
                            No homepage sections found.
                        </td>
                    </tr>
                    @endforelse
                </tbody>
            </table>
        </form>
    </div>

    <div class="space-y-6 block">
        <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
            <div>
                <h2 class="text-lg font-bold text-gray-900">{{ $discoveryLock['curated_title'] }}</h2>
                <p class="text-sm text-gray-500">Create, edit, schedule, and toggle curated projects without affecting the read-only system summary below.</p>
            </div>
            <button type="button" onclick="document.getElementById('createProjectModal').classList.remove('hidden')" class="bg-[#529F60] text-white px-4 py-2 rounded-lg text-sm font-bold shadow-sm hover:bg-[#43834F]">
                + Add Project
            </button>
        </div>

        <div class="bg-white shadow rounded-lg border border-gray-200 overflow-hidden">
            <table class="min-w-full divide-y divide-gray-200">
                <thead class="bg-gray-50">
                    <tr>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Project</th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Source</th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Status</th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Priority</th>
                        <th class="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">Actions</th>
                    </tr>
                </thead>
                <tbody class="bg-white divide-y divide-gray-200">
                    @forelse($recommendedProjects as $project)
                    <tr>
                        <td class="px-6 py-4 whitespace-nowrap">
                            <div class="flex items-center">
                                @if($project->thumbnail_url)
                                <img src="{{ $project->thumbnail_url }}" class="w-12 h-12 rounded object-cover mr-4" alt="Thumbnail">
                                @else
                                <div class="w-12 h-12 rounded bg-gray-100 flex items-center justify-center mr-4">
                                    <span class="material-symbols-outlined text-gray-400">image</span>
                                </div>
                                @endif
                                <div>
                                    <div class="text-sm font-medium text-gray-900">{{ $project->title }}</div>
                                    <div class="text-xs text-gray-500">{{ $project->project_type ?? 'N/A' }}</div>
                                </div>
                            </div>
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap">
                            <span class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-blue-100 text-blue-800">
                                {{ $project->source_type }}
                            </span>
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap">
                            @php
                                $now = now();
                                $statusLabel = 'Active';
                                $statusColor = 'bg-green-100 text-green-800';

                                if (!$project->is_active) {
                                    $statusLabel = 'Inactive';
                                    $statusColor = 'bg-gray-100 text-gray-800';
                                } elseif ($project->starts_at && $project->starts_at > $now) {
                                    $statusLabel = 'Scheduled';
                                    $statusColor = 'bg-yellow-100 text-yellow-800';
                                } elseif ($project->ends_at && $project->ends_at < $now) {
                                    $statusLabel = 'Expired';
                                    $statusColor = 'bg-red-100 text-red-800';
                                }
                            @endphp
                            <span class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full {{ $statusColor }}">
                                {{ $statusLabel }}
                            </span>
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                            {{ $project->display_priority }}
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
                            <button
                                type="button"
                                onclick="openEditProjectModal(this)"
                                data-action="{{ route('admin.recommended-projects.update', $project) }}"
                                data-title="{{ $project->title }}"
                                data-description="{{ $project->description ?? '' }}"
                                data-ratio="{{ $project->ratio ?: '16:9' }}"
                                data-project-type="{{ $project->project_type ?? '' }}"
                                data-tags="{{ implode(', ', $project->tags ?? []) }}"
                                data-modules="{{ implode(', ', $project->modules ?? []) }}"
                                data-display-priority="{{ $project->display_priority }}"
                                data-is-active="{{ $project->is_active ? '1' : '0' }}"
                                data-starts-at="{{ optional($project->starts_at)->format('Y-m-d\TH:i') }}"
                                data-ends-at="{{ optional($project->ends_at)->format('Y-m-d\TH:i') }}"
                                class="text-slate-700 hover:text-slate-900 mr-2"
                            >
                                Edit
                            </button>

                            <!-- Toggle Active Form -->
                            <form action="{{ route('admin.recommended-projects.toggle-active', $project) }}" method="POST" class="inline-block">
                                @csrf
                                @method('PATCH')
                                <button type="submit" class="{{ $project->is_active ? 'text-gray-500 hover:text-gray-700' : 'text-green-600 hover:text-green-800' }} mr-2">
                                    {{ $project->is_active ? 'Deactivate' : 'Activate' }}
                                </button>
                            </form>
                            
                            <!-- Show Now Form -->
                            <form action="{{ route('admin.recommended-projects.show-now', $project) }}" method="POST" class="inline-block" onsubmit="return confirm('Apakah Anda yakin ingin mempercepat kemunculan project ini?');">
                                @csrf
                                @method('PATCH')
                                <button type="submit" class="text-blue-600 hover:text-blue-900 mr-2 font-medium" title="Tampilkan Sekarang">Tampilkan Sekarang</button>
                            </form>
                            
                            <!-- Delete Form -->
                            <form action="{{ route('admin.recommended-projects.destroy', $project) }}" method="POST" class="inline-block" onsubmit="return confirm('Are you sure you want to delete this project?');">
                                @csrf
                                @method('DELETE')
                                <button type="submit" class="text-red-600 hover:text-red-900">Delete</button>
                            </form>
                        </td>
                    </tr>
                    @empty
                    <tr>
                        <td colspan="5" class="px-6 py-4 text-center text-sm text-gray-500">
                            No recommended projects found.
                        </td>
                    </tr>
                    @endforelse
                </tbody>
            </table>
        </div>
    </div>

    <div class="space-y-4">
        @php($systemSummaryItems = $systemDistributionSummary['items'] ?? [])
        <div class="flex flex-col gap-2 md:flex-row md:items-end md:justify-between">
            <div>
                <h2 class="text-lg font-bold text-gray-900">{{ $discoveryLock['system_section_title'] }}</h2>
                <p class="text-sm text-gray-500">{{ $discoveryLock['system_section_description'] }}</p>
            </div>
            <div class="flex flex-wrap gap-2">
                <span class="inline-flex w-fit items-center rounded-full bg-slate-900 px-3 py-1 text-xs font-semibold uppercase tracking-wide text-white">
                    Read Only
                </span>
                <span class="inline-flex w-fit items-center rounded-full bg-slate-100 px-3 py-1 text-xs font-semibold uppercase tracking-wide text-slate-700">
                    {{ $systemDistributionSummary['items_count'] ?? 0 }} sub-subject{{ ($systemDistributionSummary['items_count'] ?? 0) === 1 ? '' : 's' }}
                </span>
            </div>
        </div>

        @if($systemDistributionSummary['empty_state']['is_empty'])
        <div class="rounded-lg border border-dashed border-gray-300 bg-white px-6 py-10 text-center shadow-sm">
            <p class="text-sm font-medium text-gray-900">{{ $systemDistributionSummary['empty_state']['message'] }}</p>
            <p class="mt-2 text-xs text-gray-500">The summary will appear here after a system recommendation has been distributed to more than one distinct user.</p>
        </div>
        @else
        <div class="overflow-hidden rounded-lg border border-gray-200 bg-white shadow-sm">
            <table class="min-w-full divide-y divide-gray-200">
                <thead class="bg-gray-50">
                    <tr>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Sub-Subject</th>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Top Recommendation</th>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Subject</th>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Distinct Users</th>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Latest Distribution</th>
                        <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Source</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-gray-200 bg-white">
                    @foreach($systemSummaryItems as $item)
                    <tr>
                        <td class="px-6 py-4 align-top">
                            <div class="text-sm font-semibold text-gray-900">{{ $item['sub_subject_label'] }}</div>
                            <div class="mt-1 text-xs text-gray-500">Slug: {{ data_get($item, 'sub_subject.slug', 'n/a') }}</div>
                        </td>
                        <td class="px-6 py-4 align-top">
                            <div class="text-sm font-medium text-gray-900">{{ $item['title'] }}</div>
                            <div class="mt-1 text-xs text-gray-500">Reference: {{ $item['source_reference'] ?? 'n/a' }}</div>
                        </td>
                        <td class="px-6 py-4 align-top text-sm text-gray-600">
                            <div>{{ $item['subject_label'] }}</div>
                            <div class="mt-1 text-xs text-gray-500">{{ data_get($item, 'subject.slug', 'n/a') }}</div>
                        </td>
                        <td class="px-6 py-4 align-top text-sm font-semibold text-gray-900">
                            {{ number_format($item['distinct_user_count']) }} users
                        </td>
                        <td class="px-6 py-4 align-top text-sm text-gray-600">
                            {{ $item['latest_distribution_at_label'] }}
                        </td>
                        <td class="px-6 py-4 align-top text-sm text-gray-600">
                            <span class="inline-flex rounded-full bg-slate-100 px-3 py-1 text-xs font-semibold text-slate-700">{{ $item['source_label'] }}</span>
                        </td>
                    </tr>
                    @endforeach
                </tbody>
            </table>
        </div>
        @endif

        <div class="grid gap-4 lg:grid-cols-2">
            <div class="rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
                <h3 class="text-sm font-semibold text-gray-900">Discovery Lock Contract</h3>
                <dl class="mt-4 space-y-3 text-sm text-gray-600">
                    <div>
                        <dt class="font-medium text-gray-900">Mobile feed endpoint</dt>
                        <dd class="mt-1 font-mono text-xs text-gray-500">GET {{ $discoveryLock['feed_endpoint'] }}</dd>
                    </div>
                    <div>
                        <dt class="font-medium text-gray-900">Admin workspace</dt>
                        <dd class="mt-1 font-mono text-xs text-gray-500">{{ $discoveryLock['admin_configurator_path'] }}</dd>
                    </div>
                    <div>
                        <dt class="font-medium text-gray-900">Counted system sources</dt>
                        <dd class="mt-1 text-xs text-gray-500">{{ implode(', ', $discoveryLock['eligible_source_types']) }}</dd>
                    </div>
                    <div>
                        <dt class="font-medium text-gray-900">Eligibility</dt>
                        <dd class="mt-1 text-xs text-gray-500">Only items with distinct_user_count &gt; 1 are eligible.</dd>
                    </div>
                    <div>
                        <dt class="font-medium text-gray-900">Selection rule</dt>
                        <dd class="mt-1 text-xs text-gray-500">Exactly {{ $discoveryLock['maximum_items_per_sub_subject'] }} item is selected for each sub-subject.</dd>
                    </div>
                    <div>
                        <dt class="font-medium text-gray-900">Deterministic tie-breaker</dt>
                        <dd class="mt-1 text-xs text-gray-500">{{ implode(' -> ', $discoveryLock['tie_breakers']) }}</dd>
                    </div>
                </dl>
            </div>

            <div class="rounded-lg border border-slate-200 bg-slate-50 p-5 shadow-sm">
                <h3 class="text-sm font-semibold text-slate-900">Fallback Behavior</h3>
                <div class="mt-4 space-y-4 text-sm text-slate-600">
                    <div>
                        <p class="font-medium text-slate-900">Authenticated without enough personalization data</p>
                        <p class="mt-1 text-xs text-slate-500">{{ $discoveryLock['authenticated_fallback'] }}</p>
                    </div>
                    <div>
                        <p class="font-medium text-slate-900">Guest requests</p>
                        <p class="mt-1 text-xs text-slate-500">{{ $discoveryLock['guest_fallback'] }}</p>
                    </div>
                </div>
            </div>
        </div>
    </div>
</div>

<!-- Create Project Modal -->
<div id="createProjectModal" class="hidden fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-900/60 backdrop-blur-sm" aria-labelledby="modal-title" role="dialog" aria-modal="true">
    <!-- Background overlay -->
    <div class="absolute inset-0" aria-hidden="true" onclick="document.getElementById('createProjectModal').classList.add('hidden')"></div>

    <div class="relative bg-white border border-slate-950 w-full max-w-2xl shadow-[8px_8px_0px_0px_rgba(0,0,0,1)] flex flex-col max-h-[90vh]">
        <!-- Modal Header -->
        <div class="flex justify-between items-center px-4 py-3 border-b border-slate-200 bg-slate-50 shrink-0">
            <h2 class="text-sm font-bold uppercase tracking-wider text-slate-800" id="modal-title">Add Recommended Project</h2>
            <button type="button" class="text-slate-400 hover:text-slate-900 transition-colors" onclick="document.getElementById('createProjectModal').classList.add('hidden')">
                <span class="material-symbols-outlined">close</span>
            </button>
        </div>

        <!-- Modal Content (High Density Form) -->
        <form action="{{ route('admin.recommended-projects.store') }}" method="POST" enctype="multipart/form-data" class="flex flex-col overflow-hidden h-full" id="addProjectForm" onsubmit="handleProjectSubmit(event)">
            @csrf
            <div class="p-4 grid grid-cols-12 gap-y-4 gap-x-6 overflow-y-auto">
                <!-- Project Title -->
                <div class="col-span-12">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Project Title</label>
                    <input type="text" name="title" required class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" placeholder="Enter high-level project name"/>
                </div>

                <!-- Description -->
                <div class="col-span-12">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Description</label>
                    <textarea name="description" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none h-20 resize-none" placeholder="Brief technical summary..."></textarea>
                </div>

                <!-- Project Type & Ratio -->
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Project Type</label>
                    <input type="text" name="project_type" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" placeholder="e.g. mobile, web"/>
                </div>
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Ratio</label>
                    <select name="ratio" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none appearance-none bg-white">
                        <option value="16:9">16:9</option>
                        <option value="1:1">1:1</option>
                        <option value="4:3">4:3</option>
                    </select>
                </div>

                <!-- Tags -->
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Tags (Comma Separated)</label>
                    <input type="text" name="tags" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" placeholder="cloud, automation, security"/>
                </div>
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Modules (Comma Separated)</label>
                    <input type="text" name="modules" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" placeholder="Discovery, Build, Review"/>
                </div>

                <!-- Thumbnail & Document -->
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Thumbnail Image</label>
                    <div class="relative border border-slate-300 p-2 flex items-center bg-slate-50 overflow-hidden h-[46px] group hover:bg-slate-100 transition-colors">
                        <input type="file" name="thumbnail" accept="image/*" class="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-10" onchange="updateFileLabel(this, 'thumb-label', 'thumb-icon', 'thumb-btn')">
                        <span class="material-symbols-outlined text-slate-400 mr-2 group-hover:text-slate-600 transition-colors" id="thumb-icon">image</span>
                        <span class="text-xs text-slate-600 truncate font-mono flex-1" id="thumb-label">Select image...</span>
                        <button class="ml-auto text-[10px] border border-slate-900 px-2 py-0.5 hover:bg-slate-200 uppercase font-bold relative z-0 shrink-0" type="button" id="thumb-btn">BROWSE</button>
                    </div>
                </div>
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Project Document</label>
                    <div class="relative border border-slate-300 p-2 flex items-center bg-slate-50 overflow-hidden h-[46px] group hover:bg-slate-100 transition-colors">
                        <input type="file" name="project_file" accept=".pdf,.ppt,.pptx,.doc,.docx" class="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-10" onchange="updateFileLabel(this, 'doc-label', 'doc-icon', 'doc-btn')">
                        <span class="material-symbols-outlined text-slate-400 mr-2 group-hover:text-slate-600 transition-colors" id="doc-icon">description</span>
                        <span class="text-xs text-slate-600 truncate font-mono flex-1" id="doc-label">Upload PDF, PPT, DOC</span>
                        <button class="ml-auto text-[10px] border border-slate-900 px-2 py-0.5 hover:bg-slate-200 uppercase font-bold relative z-0 shrink-0" type="button" id="doc-btn">UPLOAD</button>
                    </div>
                </div>

                <!-- Display Priority & Active Status -->
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Display Priority</label>
                    <input type="number" name="display_priority" value="0" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none"/>
                </div>
                <div class="col-span-6 flex items-end">
                    <label class="flex items-center gap-3 cursor-pointer h-[34px]">
                        <input type="checkbox" name="is_active" value="1" checked class="w-4 h-4 border-slate-300 text-slate-900 focus:ring-0 rounded-none"/>
                        <span class="text-[10px] font-bold uppercase text-slate-700">Project Is Active</span>
                    </label>
                </div>

                <!-- Start & End Dates -->
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Starts At (Optional)</label>
                    <div class="relative">
                        <input type="datetime-local" name="starts_at" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none"/>
                    </div>
                </div>
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Ends At (Optional)</label>
                    <div class="relative">
                        <input type="datetime-local" name="ends_at" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none"/>
                    </div>
                </div>
            </div>

            <!-- Modal Footer Actions -->
            <div class="flex items-center justify-end gap-3 px-4 py-4 border-t border-slate-200 bg-slate-50 mt-auto shrink-0">
                <button type="button" class="px-4 py-2 text-xs font-bold uppercase tracking-widest text-slate-500 bg-white border border-slate-300 hover:bg-slate-100 transition-colors" onclick="document.getElementById('createProjectModal').classList.add('hidden')">Discard</button>
                <button type="submit" id="submitProjectBtn" data-submit-button data-loading-label="UPLOADING..." class="px-6 py-2 text-xs font-bold uppercase tracking-widest text-white bg-slate-950 border border-slate-950 hover:bg-slate-800 transition-colors">Save Project</button>
            </div>
        </form>
    </div>
</div>

<div id="editProjectModal" class="hidden fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-900/60 backdrop-blur-sm" aria-labelledby="edit-modal-title" role="dialog" aria-modal="true">
    <div class="absolute inset-0" aria-hidden="true" onclick="closeEditProjectModal()"></div>

    <div class="relative bg-white border border-slate-950 w-full max-w-2xl shadow-[8px_8px_0px_0px_rgba(0,0,0,1)] flex flex-col max-h-[90vh]">
        <div class="flex justify-between items-center px-4 py-3 border-b border-slate-200 bg-slate-50 shrink-0">
            <h2 class="text-sm font-bold uppercase tracking-wider text-slate-800" id="edit-modal-title">Edit Recommended Project</h2>
            <button type="button" class="text-slate-400 hover:text-slate-900 transition-colors" onclick="closeEditProjectModal()">
                <span class="material-symbols-outlined">close</span>
            </button>
        </div>

        <form method="POST" enctype="multipart/form-data" class="flex flex-col overflow-hidden h-full" id="editProjectForm" onsubmit="handleProjectSubmit(event)">
            @csrf
            @method('PUT')

            <div class="p-4 grid grid-cols-12 gap-y-4 gap-x-6 overflow-y-auto">
                <div class="col-span-12">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Project Title</label>
                    <input type="text" id="editProjectTitle" name="title" required class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" />
                </div>

                <div class="col-span-12">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Description</label>
                    <textarea id="editProjectDescription" name="description" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none h-20 resize-none"></textarea>
                </div>

                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Project Type</label>
                    <input type="text" id="editProjectType" name="project_type" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" />
                </div>
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Ratio</label>
                    <select id="editProjectRatio" name="ratio" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none appearance-none bg-white">
                        <option value="16:9">16:9</option>
                        <option value="1:1">1:1</option>
                        <option value="4:3">4:3</option>
                    </select>
                </div>

                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Tags (Comma Separated)</label>
                    <input type="text" id="editProjectTags" name="tags" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" />
                </div>
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Modules (Comma Separated)</label>
                    <input type="text" id="editProjectModules" name="modules" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" />
                </div>

                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Thumbnail Image</label>
                    <div class="relative border border-slate-300 p-2 flex items-center bg-slate-50 overflow-hidden h-[46px] group hover:bg-slate-100 transition-colors">
                        <input type="file" id="editProjectThumbnail" name="thumbnail" accept="image/*" class="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-10" onchange="updateFileLabel(this, 'edit-thumb-label', 'edit-thumb-icon', 'edit-thumb-btn', 'Keep current image')">
                        <span class="material-symbols-outlined text-slate-400 mr-2 group-hover:text-slate-600 transition-colors" id="edit-thumb-icon">image</span>
                        <span class="text-xs text-slate-600 truncate font-mono flex-1" id="edit-thumb-label">Keep current image</span>
                        <button class="ml-auto text-[10px] border border-slate-900 px-2 py-0.5 hover:bg-slate-200 uppercase font-bold relative z-0 shrink-0" type="button" id="edit-thumb-btn">BROWSE</button>
                    </div>
                </div>
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Project Document</label>
                    <div class="relative border border-slate-300 p-2 flex items-center bg-slate-50 overflow-hidden h-[46px] group hover:bg-slate-100 transition-colors">
                        <input type="file" id="editProjectFile" name="project_file" accept=".pdf,.ppt,.pptx,.doc,.docx" class="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-10" onchange="updateFileLabel(this, 'edit-doc-label', 'edit-doc-icon', 'edit-doc-btn', 'Keep current document')">
                        <span class="material-symbols-outlined text-slate-400 mr-2 group-hover:text-slate-600 transition-colors" id="edit-doc-icon">description</span>
                        <span class="text-xs text-slate-600 truncate font-mono flex-1" id="edit-doc-label">Keep current document</span>
                        <button class="ml-auto text-[10px] border border-slate-900 px-2 py-0.5 hover:bg-slate-200 uppercase font-bold relative z-0 shrink-0" type="button" id="edit-doc-btn">UPLOAD</button>
                    </div>
                </div>

                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Display Priority</label>
                    <input type="number" id="editProjectPriority" name="display_priority" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none" />
                </div>
                <div class="col-span-6 flex items-end">
                    <label class="flex items-center gap-3 cursor-pointer h-[34px]">
                        <input type="checkbox" id="editProjectIsActive" name="is_active" value="1" class="w-4 h-4 border-slate-300 text-slate-900 focus:ring-0 rounded-none"/>
                        <span class="text-[10px] font-bold uppercase text-slate-700">Project Is Active</span>
                    </label>
                </div>

                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Starts At (Optional)</label>
                    <input type="datetime-local" id="editProjectStartsAt" name="starts_at" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none"/>
                </div>
                <div class="col-span-6">
                    <label class="block text-[10px] font-bold uppercase text-slate-500 mb-1">Ends At (Optional)</label>
                    <input type="datetime-local" id="editProjectEndsAt" name="ends_at" class="w-full border border-slate-300 px-3 py-1.5 text-sm focus:ring-0 focus:border-slate-900 rounded-none"/>
                </div>
            </div>

            <div class="flex items-center justify-end gap-3 px-4 py-4 border-t border-slate-200 bg-slate-50 mt-auto shrink-0">
                <button type="button" class="px-4 py-2 text-xs font-bold uppercase tracking-widest text-slate-500 bg-white border border-slate-300 hover:bg-slate-100 transition-colors" onclick="closeEditProjectModal()">Cancel</button>
                <button type="submit" id="submitEditProjectBtn" data-submit-button data-loading-label="SAVING..." class="px-6 py-2 text-xs font-bold uppercase tracking-widest text-white bg-slate-950 border border-slate-950 hover:bg-slate-800 transition-colors">Save Changes</button>
            </div>
        </form>
    </div>
</div>
@endsection

@push('scripts')
<script>
    function updateFileLabel(input, labelId, iconId, btnId, emptyLabel = null) {
        const label = document.getElementById(labelId);
        const icon = document.getElementById(iconId);
        const btn = document.getElementById(btnId);
        const defaultLabel = emptyLabel ?? (labelId === 'thumb-label' ? 'Select image...' : 'Upload PDF, PPT, DOC');
        const defaultIcon = labelId.includes('thumb') ? 'image' : 'description';
        const defaultButton = labelId.includes('thumb') ? 'BROWSE' : 'UPLOAD';

        if (input.files && input.files.length > 0) {
            label.textContent = input.files[0].name;
            label.classList.add('text-blue-600', 'font-bold');
            
            icon.textContent = 'check_circle';
            icon.classList.add('text-[#529F60]');
            icon.classList.remove('text-slate-400');
            
            btn.textContent = 'REPLACE';
        } else {
            label.textContent = defaultLabel;
            label.classList.remove('text-blue-600', 'font-bold');
            
            icon.textContent = defaultIcon;
            icon.classList.remove('text-[#529F60]');
            icon.classList.add('text-slate-400');
            
            btn.textContent = defaultButton;
        }
    }

    function handleProjectSubmit(e) {
        const btn = e.target.querySelector('[data-submit-button]');

        if (!btn) {
            return;
        }

        btn.innerHTML = '<span class="material-symbols-outlined text-[14px] animate-spin mr-2 align-middle">progress_activity</span> ' + (btn.dataset.loadingLabel || 'SAVING...');
        btn.classList.add('opacity-80', 'cursor-wait');
        btn.classList.remove('hover:bg-slate-800');
        btn.style.pointerEvents = 'none';
    }

    function openEditProjectModal(button) {
        const form = document.getElementById('editProjectForm');
        form.action = button.dataset.action;

        document.getElementById('editProjectTitle').value = button.dataset.title || '';
        document.getElementById('editProjectDescription').value = button.dataset.description || '';
        document.getElementById('editProjectRatio').value = button.dataset.ratio || '16:9';
        document.getElementById('editProjectType').value = button.dataset.projectType || '';
        document.getElementById('editProjectTags').value = button.dataset.tags || '';
        document.getElementById('editProjectModules').value = button.dataset.modules || '';
        document.getElementById('editProjectPriority').value = button.dataset.displayPriority || 0;
        document.getElementById('editProjectIsActive').checked = button.dataset.isActive === '1';
        document.getElementById('editProjectStartsAt').value = button.dataset.startsAt || '';
        document.getElementById('editProjectEndsAt').value = button.dataset.endsAt || '';

        document.getElementById('editProjectThumbnail').value = '';
        document.getElementById('editProjectFile').value = '';
        updateFileLabel(document.getElementById('editProjectThumbnail'), 'edit-thumb-label', 'edit-thumb-icon', 'edit-thumb-btn', 'Keep current image');
        updateFileLabel(document.getElementById('editProjectFile'), 'edit-doc-label', 'edit-doc-icon', 'edit-doc-btn', 'Keep current document');

        document.getElementById('editProjectModal').classList.remove('hidden');
    }

    function closeEditProjectModal() {
        document.getElementById('editProjectModal').classList.add('hidden');
    }
</script>
@endpush