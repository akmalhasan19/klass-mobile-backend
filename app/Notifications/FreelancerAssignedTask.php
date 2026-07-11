<?php

namespace App\Notifications;

use App\Models\MarketplaceTask;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Notifications\Messages\MailMessage;
use Illuminate\Notifications\Notification;

class FreelancerAssignedTask extends Notification implements ShouldQueue
{
    use Queueable;

    public function __construct(public MarketplaceTask $task)
    {
    }

    public function via(object $notifiable): array
    {
        return ['mail', 'database']; // Assuming standard notification channels
    }

    public function toMail(object $notifiable): MailMessage
    {
        return (new MailMessage)
                    ->subject('Task Perbaikan Media Baru (Assigned)')
                    ->line('Anda telah ditugaskan sebuah task perbaikan media karena Anda adalah kandidat terbaik yang kami temukan.')
                    ->action('Lihat Detail Task', url('/tasks/' . $this->task->id))
                    ->line('Mohon segera review dan mulai kerjakan instruksi dari pengajar.');
    }

    public function toArray(object $notifiable): array
    {
        return [
            'task_id' => $this->task->id,
            'type' => 'assigned_task',
        ];
    }
}
