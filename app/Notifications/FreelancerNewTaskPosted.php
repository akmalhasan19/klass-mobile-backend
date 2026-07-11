<?php

namespace App\Notifications;

use App\Models\MarketplaceTask;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Notifications\Messages\MailMessage;
use Illuminate\Notifications\Notification;

class FreelancerNewTaskPosted extends Notification implements ShouldQueue
{
    use Queueable;

    public function __construct(public MarketplaceTask $task)
    {
    }

    public function via(object $notifiable): array
    {
        return ['database']; // Broadcast to all eligible freelancers, typically app-notification
    }

    public function toMail(object $notifiable): MailMessage
    {
        return (new MailMessage)
                    ->subject('Proyek Perbaikan Media Publik Baru')
                    ->line('Ada tugas perbaikan media publik baru yang tersedia untuk penawaran (bidding).')
                    ->action('Lihat Detail', url('/marketplace/tasks/' . $this->task->id));
    }

    public function toArray(object $notifiable): array
    {
        return [
            'task_id' => $this->task->id,
            'type' => 'new_public_task',
        ];
    }
}
