<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\ActivityLog;
use App\Models\User;
use Illuminate\Http\Request;
use Illuminate\View\View;

class AdminUserController extends Controller
{
    /**
     * Display a listing of users.
     */
    public function index(Request $request): View
    {
        $search = $request->query('search');

        $users = User::query()
            ->when($search, function ($query, $search) {
                $query->where('name', 'like', "%{$search}%")
                      ->orWhere('email', 'like', "%{$search}%");
            })
            ->latest()
            ->paginate(15)
            ->withQueryString();

        return view('admin.users.index', compact('users', 'search'));
    }

    /**
     * Display user details.
     */
    public function show(User $user): View
    {
        return view('admin.users.show', compact('user'));
    }

    /**
     * Update user role.
     */
    public function updateRole(Request $request, User $user)
    {
        $request->validate([
            'role' => 'required|in:admin,user',
        ]);

        $oldRole = $user->role;
        $newRole = $request->role;

        if ($oldRole !== $newRole) {
            $user->update(['role' => $newRole]);

            ActivityLog::create([
                'actor_id'     => auth()->id(),
                'action'       => 'update_role',
                'subject_type' => User::class,
                'subject_id'   => $user->id,
                'metadata'     => [
                    'old_role' => $oldRole,
                    'new_role' => $newRole,
                ],
            ]);
        }

        return back()->with('success', 'Role pengguna berhasil diperbarui.');
    }
}
