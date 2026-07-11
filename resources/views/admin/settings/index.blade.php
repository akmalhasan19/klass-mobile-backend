@extends('admin.layouts.app')

@section('title', 'System Settings')
@section('page-title', 'System Settings')
@section('page-description', 'Kelola konfigurasi global untuk aplikasi Klass.')

@section('content')
<div class="h-full p-8 overflow-y-auto">
    <div class="max-w-[1000px] mx-auto space-y-8 pb-10">
        
        <form action="{{ route('admin.settings.update') }}" method="POST" class="space-y-8">
            @csrf
            @method('PATCH')

            @foreach($settings as $group => $items)
            <div class="bg-surface border-2 border-black p-8 shadow-drag relative overflow-hidden">
                {{-- Decorative background text for groups --}}
                <div class="absolute -top-4 -right-4 text-[60px] font-black text-black/5 uppercase select-none pointer-events-none">
                    {{ $group }}
                </div>
                
                <h3 class="text-[18px] font-black uppercase tracking-tight text-text-main mb-6 border-b-2 border-black pb-2 inline-block">
                    {{ str_replace('_', ' ', $group) }}
                </h3>

                <div class="grid grid-cols-1 gap-8 mt-4">
                    @foreach($items as $setting)
                    <div class="flex flex-col gap-2">
                        <label for="setting_{{ $setting->key }}" class="font-bold text-[14px] text-text-main flex items-center gap-2">
                            {{ strtoupper(str_replace('_', ' ', $setting->key)) }}
                            @if($setting->type === 'boolean')
                                <span class="bg-primary/20 text-primary border border-primary px-1.5 py-0.5 text-[9px] font-black uppercase">SWITCH</span>
                            @else
                                <span class="bg-gray-100 text-text-muted border border-border px-1.5 py-0.5 text-[9px] font-black uppercase tracking-widest">{{ $setting->type }}</span>
                            @endif
                        </label>
                        
                        @if($setting->type === 'boolean')
                        <div class="flex items-center gap-4">
                            <label class="relative inline-flex items-center cursor-pointer">
                                <input type="hidden" name="settings[{{ $setting->key }}]" value="0">
                                <input type="checkbox" name="settings[{{ $setting->key }}]" value="1" {{ $setting->value == '1' ? 'checked' : '' }} class="sr-only peer">
                                <div class="w-14 h-8 bg-white border-2 border-black peer-focus:outline-none rounded-none peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-[4px] after:left-[4px] after:bg-black after:border-black after:h-6 after:w-6 after:transition-all peer-checked:bg-primary shadow-[2px_2px_0px_rgba(0,0,0,1)]"></div>
                            </label>
                            <span class="text-[13px] font-medium text-text-muted italic">{{ $setting->description }}</span>
                        </div>
                        @elseif($setting->type === 'text')
                        <div class="space-y-1">
                            <input type="text" id="setting_{{ $setting->key }}" name="settings[{{ $setting->key }}]" value="{{ $setting->value }}" class="w-full bg-white border-2 border-black p-3 text-[14px] font-medium focus:ring-primary focus:border-primary placeholder:text-gray-300">
                            <p class="text-[11px] mono-text text-text-muted">{{ $setting->description }}</p>
                        </div>
                        @endif
                    </div>
                    @endforeach
                </div>
            </div>
            @endforeach

            <div class="flex justify-end pt-4">
                <button type="submit" class="bg-primary hover:bg-primary/90 text-black border-2 border-black font-black px-12 py-4 shadow-drag transition active:translate-x-[2px] active:translate-y-[2px] active:shadow-none uppercase tracking-widest">
                    Simpan Perubahan
                </button>
            </div>
        </form>

    </div>
</div>
@endsection
