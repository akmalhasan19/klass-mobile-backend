<div class="relative flex h-auto min-h-screen w-full max-w-[240px] flex-col bg-surface border-r border-border shrink-0 z-20">
    <div class="layout-container flex h-full grow flex-col">
        <div class="flex flex-1 justify-center py-5">
            <div class="layout-content-container flex flex-col w-full flex-1">
                <div class="flex h-full flex-col justify-between bg-surface p-4">
                    <div class="flex flex-col gap-4">
                        <h1 class="text-text-main text-lg font-bold leading-normal pb-6 border-b border-border tracking-tight uppercase">KLASS</h1>
                        
                        <div class="flex flex-col gap-1 mt-2">
                            <x-admin.nav-item
                                :href="route('admin.dashboard')"
                                :active="request()->routeIs('admin.dashboard')"
                                label="Dashboard"
                                icon="grid_view"
                            />

                            <x-admin.nav-item
                                :href="route('admin.users.index')"
                                :active="request()->routeIs('admin.users.*')"
                                label="User Management"
                                icon="group"
                            />
                            
                            <x-admin.nav-item
                                :href="route('admin.homepage-sections.index')"
                                :active="request()->routeIs('admin.homepage-sections.*')"
                                label="Homepage Configurator"
                                icon="view_quilt"
                            />

                            <x-admin.nav-item
                                :href="route('admin.tasks.index')"
                                :active="request()->routeIs('admin.tasks.*')"
                                label="Marketplace Tasks"
                                icon="task_alt"
                            />

                            <x-admin.nav-item
                                :href="route('admin.topics.index')"
                                :active="request()->routeIs(['admin.topics.*', 'admin.contents.*'])"
                                label="Content & Topics"
                                icon="menu_book"
                            />

                            <x-admin.nav-item
                                href="{{ route('admin.media.index') }}"
                                :active="request()->routeIs('admin.media.*')"
                                label="Media Library"
                                icon="photo_library"
                            />

                            <x-admin.nav-item
                                :href="route('admin.media-generations.index')"
                                :active="request()->routeIs('admin.media-generations.*')"
                                label="Media Generation"
                                icon="animation"
                            />

                            <x-admin.nav-item
                                :href="route('admin.activity-logs.index')"
                                :active="request()->routeIs('admin.activity-logs.*')"
                                label="Activity Logs"
                                icon="monitoring"
                            />

                            <x-admin.nav-item
                                :href="route('admin.settings.index')"
                                :active="request()->routeIs('admin.settings.*')"
                                label="System Settings"
                                icon="settings"
                                class="mt-4"
                            />
                        </div>
                    </div>

                    <div class="mt-auto pt-4 border-t border-border flex items-center justify-between gap-3">
                        <div class="flex items-center gap-3">
                            <div class="w-8 h-8 bg-gray-200 overflow-hidden border border-border">
                                <img alt="{{ Auth::user()->name ?? 'Admin user' }}" class="w-full h-full object-cover" src="https://ui-avatars.com/api/?name={{ urlencode(Auth::user()->name ?? 'Admin') }}&background=0D8ABC&color=fff"/>
                            </div>
                            <div class="flex flex-col">
                                <span class="text-[12px] font-semibold text-text-main">{{ Auth::user()->name ?? 'Admin User' }}</span>
                                <span class="text-[11px] mono-text text-text-muted">ID:A-{{ Auth::user()->id ?? '9921' }}</span>
                            </div>
                        </div>
                        <form method="POST" action="{{ route('admin.logout') }}">
                            @csrf
                            <button type="submit" class="text-text-muted hover:text-red-500 transition-colors flex items-center" title="Logout">
                                <span class="material-symbols-outlined" style="font-size: 18px;">logout</span>
                            </button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    </div>
</div>
