{{--
    Admin Badge Status Component
    Usage: @include('admin.partials.badge', ['status' => 'active', 'label' => 'Aktif'])

    Supported $status values: active, inactive, pending, rejected, published, draft, admin, user
--}}
@php
$map = [
    'active'    => 'bg-emerald-500/10 text-emerald-400 ring-1 ring-emerald-500/20',
    'inactive'  => 'bg-slate-500/10 text-slate-400 ring-1 ring-slate-500/20',
    'pending'   => 'bg-amber-500/10 text-amber-400 ring-1 ring-amber-500/20',
    'rejected'  => 'bg-red-500/10 text-red-400 ring-1 ring-red-500/20',
    'published' => 'bg-emerald-500/10 text-emerald-400 ring-1 ring-emerald-500/20',
    'draft'     => 'bg-slate-500/10 text-slate-400 ring-1 ring-slate-500/20',
    'admin'     => 'bg-indigo-500/10 text-indigo-400 ring-1 ring-indigo-500/20',
    'user'      => 'bg-slate-500/10 text-slate-400 ring-1 ring-slate-500/20',
];
$classes = $map[$status ?? 'inactive'] ?? 'bg-slate-500/10 text-slate-400 ring-1 ring-slate-500/20';
@endphp
<span class="inline-flex items-center px-2 py-0.5 rounded-md text-xs font-medium {{ $classes }}">
    {{ $label ?? $status }}
</span>
