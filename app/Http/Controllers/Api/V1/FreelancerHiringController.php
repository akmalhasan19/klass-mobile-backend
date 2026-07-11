<?php

namespace App\Http\Controllers\Api\V1;

use App\Http\Controllers\Controller;
use App\Http\Requests\HireFreelancerRequest;
use App\Http\Traits\ApiResponseTrait;
use App\MediaGeneration\MediaGenerationApiException;
use App\Models\MarketplaceTask;
use App\Models\MediaGeneration;
use Illuminate\Http\JsonResponse;

class FreelancerHiringController extends Controller
{
    use ApiResponseTrait;

    public function hire(HireFreelancerRequest $request, string $mediaGeneration): JsonResponse
    {
        $teacher = $request->user();

        if (! $teacher || ! $teacher->isTeacher()) {
            throw MediaGenerationApiException::teacherRoleRequired();
        }

        $generation = MediaGeneration::query()
            ->whereKey($mediaGeneration)
            ->where('teacher_id', $teacher->id)
            ->first();

        if (! $generation) {
            throw MediaGenerationApiException::notFound();
        }

        if (! $generation->isTerminal()) {
            return $this->error(
                'Media generation belum selesai. Tidak dapat membuka task untuk media yang sedang diproses.',
                422
            );
        }

        // Must have an associated content to attach physical task. If purely generation without content yet,
        // we fallback or require it, but for our system design generations are tied to content.
        if (! $generation->content_id) {
            return $this->error('Media generation ini tidak terhubung dengan konten apapun.', 422);
        }

        $mode = $request->input('mode');
        $refinementDescription = $request->validated('refinement_description');

        $taskAttributes = [
            'content_id' => $generation->content_id,
            'media_generation_id' => $generation->id,
            'creator_id' => $teacher->id,
            'description' => $refinementDescription,
        ];

        if ($mode === 'auto_suggest') {
            $freelancerId = $request->validated('selected_freelancer_id');
            
            // Verifikasi opsional: memastikan freelancer ada di top matches (FreelancerMatch table)
            // (Dilewati untuk kemudahan jika validation `exists` sudah cukup secara fungsional)

            $taskAttributes['task_type'] = MarketplaceTask::TYPE_SUGGESTION;
            $taskAttributes['status'] = MarketplaceTask::STATUS_ASSIGNED;
            $taskAttributes['suggested_freelancer_id'] = $freelancerId;

            $task = MarketplaceTask::create($taskAttributes);

            // Mockup: TODO: Send notification to the suggested freelancer
            // event(new \App\Events\FreelancerAssignedTask($task, $freelancerId));

            return $this->created(
                $task,
                'Task perbaikan berhasil diassign langsung ke freelancer.'
            );
        } else {
            // manual_task (open bid)
            $taskAttributes['task_type'] = MarketplaceTask::TYPE_BID;
            $taskAttributes['status'] = MarketplaceTask::STATUS_OPEN_FOR_BID;

            $task = MarketplaceTask::create($taskAttributes);

            // Mockup: TODO: Broadcast notification to eligible freelancers
            // event(new \App\Events\FreelancerNewTaskPosted($task));

            return $this->created(
                $task,
                'Task perbaikan telah diposting ke publik untuk bidding.'
            );
        }
    }
}
