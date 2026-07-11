@props(['href', 'active', 'label', 'icon', 'class' => ''])

@php
$activeClass = 'bg-gray-100 border-l-2 border-black';
$inactiveClass = 'hover:bg-gray-50 transition-colors border-l-2 border-transparent';
$baseClass = 'flex items-center gap-3 px-3 py-2 group ' . $class;
@endphp

<a href="{{ $href }}"
    class="{{ $baseClass }} {{ $active ? $activeClass : $inactiveClass }}">
    <div class="{{ $active ? 'text-text-main' : 'text-text-muted group-hover:text-text-main transition-colors' }} flex items-center justify-center">
        <span class="material-symbols-outlined" style="font-size: 18px; {{ $active ? 'font-variation-settings: \'FILL\' 1;' : '' }}">{{ $icon }}</span>
    </div>
    <span class="{{ $active ? 'text-text-main' : 'text-text-muted group-hover:text-text-main' }} text-[13px] font-medium leading-normal">{{ $label }}</span>
</a>
