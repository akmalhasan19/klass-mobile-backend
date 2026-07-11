{{--
    Admin Table Wrapper
    Usage:
        @include('admin.partials.table', [
            'headers' => ['Nama', 'Email', 'Role', 'Aksi'],
            'empty'   => $users->isEmpty(),
            'slot'    => ...  (gunakan @slot jika perlu, atau langsung isi $slot)
        ])

    Bungkus baris tabel di @include dalam $slot.
    Lebih mudah: pakai partial ini hanya sebagai wrapper dan isi <tbody> langsung di view parent.
--}}
<div class="overflow-hidden rounded-xl border border-slate-800 bg-slate-900">
    <div class="overflow-x-auto">
        <table class="w-full text-sm text-left">
            <thead>
                <tr class="border-b border-slate-800 bg-slate-800/50">
                    @foreach($headers as $header)
                    <th class="px-4 py-3 text-xs font-semibold text-slate-400 uppercase tracking-wider whitespace-nowrap">
                        {{ $header }}
                    </th>
                    @endforeach
                </tr>
            </thead>
            <tbody class="divide-y divide-slate-800/60">
                {{ $slot }}
            </tbody>
        </table>
    </div>
</div>
