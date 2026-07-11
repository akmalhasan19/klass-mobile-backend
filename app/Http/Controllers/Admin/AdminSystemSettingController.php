<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\SystemSetting;
use Illuminate\Http\Request;

class AdminSystemSettingController extends Controller
{
    /**
     * Display a listing of system settings.
     */
    public function index()
    {
        $settings = SystemSetting::all()->groupBy('group');
        
        return view('admin.settings.index', compact('settings'));
    }

    /**
     * Update the specified system settings.
     */
    public function update(Request $request)
    {
        $validatedData = $request->validate([
            'settings' => 'required|array',
        ]);

        foreach ($validatedData['settings'] as $key => $value) {
            $setting = SystemSetting::where('key', $key)->first();
            
            if ($setting) {
                // Handle boolean types from checkboxes/toggles
                if ($setting->type === 'boolean' || $setting->type === 'bool') {
                    $value = ($value === '1' || $value === 'on' || $value === true || $value === 'true') ? '1' : '0';
                }
                
                $setting->update(['value' => $value]);
            }
        }

        return redirect()->route('admin.settings.index')
            ->with('success', 'Pengaturan sistem berhasil diperbarui.');
    }
}
