<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\MediaGeneration;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Http\Request;
use Illuminate\View\View;

class AdminMediaGenerationController extends Controller
{
    public function index(Request $request): View
    {
        $search = trim((string) $request->query('search', ''));
        $requestedStatus = trim((string) $request->query('status', ''));
        $statuses = MediaGenerationLifecycle::all();
        $status = in_array($requestedStatus, $statuses, true) ? $requestedStatus : '';

        $mediaGenerations = MediaGeneration::query()
            ->with(['teacher', 'subject', 'subSubject.subject'])
            ->when($status !== '', fn (Builder $query): Builder => $query->where('status', $status))
            ->when($search !== '', function (Builder $query) use ($search): Builder {
                return $query->where(function (Builder $innerQuery) use ($search): void {
                    $innerQuery
                        ->where('id', 'like', '%' . $search . '%')
                        ->orWhere('raw_prompt', 'like', '%' . $search . '%')
                        ->orWhereHas('teacher', function (Builder $teacherQuery) use ($search): void {
                            $teacherQuery
                                ->where('name', 'like', '%' . $search . '%')
                                ->orWhere('email', 'like', '%' . $search . '%');
                        });
                });
            })
            ->recentFirst()
            ->paginate(15)
            ->withQueryString();

        $selectedGenerationId = trim((string) $request->query('generation', ''));

        if ($selectedGenerationId === '' && $mediaGenerations->isNotEmpty()) {
            $selectedGenerationId = (string) $mediaGenerations->first()->id;
        }

        return view('admin.media-generations.index', [
            'mediaGenerations' => $mediaGenerations,
            'search' => $search,
            'status' => $status,
            'statuses' => $statuses,
            'selectedGenerationId' => $selectedGenerationId,
        ]);
    }
}