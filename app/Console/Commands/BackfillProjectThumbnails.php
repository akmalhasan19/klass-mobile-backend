<?php

namespace App\Console\Commands;

use App\Models\RecommendedProject;
use App\Services\FileUploadService;
use App\Services\ThumbnailGeneratorService;
use Illuminate\Console\Command;
use Illuminate\Http\UploadedFile;

/**
 * Backfill thumbnails untuk RecommendedProject yang sudah ada.
 *
 * Mencari semua project yang memiliki project_file_url tapi thumbnail_url null,
 * lalu otomatis generate thumbnail dari file dokumen tersebut.
 *
 * Usage: php artisan projects:backfill-thumbnails
 */
class BackfillProjectThumbnails extends Command
{
    protected $signature = 'projects:backfill-thumbnails
                            {--dry-run : Tampilkan project yang akan diproses tanpa melakukan perubahan}
                            {--force : Proses tanpa konfirmasi}';

    protected $description = 'Backfill auto-generated thumbnails for recommended projects that have a project file but no thumbnail.';

    public function handle(
        ThumbnailGeneratorService $thumbnailGenerator,
        FileUploadService $fileUploadService,
    ): int {
        $query = RecommendedProject::query()
            ->whereNotNull('project_file_url')
            ->where('project_file_url', '!=', '')
            ->where(function ($q) {
                $q->whereNull('thumbnail_url')
                  ->orWhere('thumbnail_url', '');
            });

        $total = $query->count();

        if ($total === 0) {
            $this->info('✅ No projects found that need thumbnail backfilling.');
            return self::SUCCESS;
        }

        $this->info("Found {$total} project(s) with project_file but no thumbnail.");

        if ($this->option('dry-run')) {
            $this->table(
                ['ID', 'Title', 'Project File URL'],
                $query->get(['id', 'title', 'project_file_url'])->map(fn ($p) => [
                    $p->id, $p->title, $p->project_file_url,
                ])->toArray()
            );
            $this->info('Dry-run complete. No changes made.');
            return self::SUCCESS;
        }

        if (! $this->option('force') && ! $this->confirm("Proceed to generate thumbnails for {$total} project(s)?")) {
            $this->info('Cancelled.');
            return self::SUCCESS;
        }

        $bar = $this->output->createProgressBar($total);
        $bar->start();

        $success = 0;
        $failed = 0;

        $query->chunk(10, function ($projects) use ($thumbnailGenerator, $fileUploadService, $bar, &$success, &$failed) {
            /** @var \App\Models\RecommendedProject $project */
            foreach ($projects as $project) {
                try {
                    $thumbnailPath = $thumbnailGenerator->generateFromUrl($project->project_file_url);

                    if ($thumbnailPath === null) {
                        $this->newLine();
                        $this->warn("⚠️  Could not generate thumbnail for: {$project->title} (ID: {$project->id})");
                        $failed++;
                        $bar->advance();
                        continue;
                    }

                    // Upload thumbnail ke Supabase storage
                    $extension = pathinfo($thumbnailPath, PATHINFO_EXTENSION) ?: 'png';
                    $uploadedFile = new UploadedFile(
                        $thumbnailPath,
                        'backfill_thumb_' . $project->id . '.' . $extension,
                        mime_content_type($thumbnailPath),
                        null,
                        true
                    );

                    $upload = $fileUploadService->upload($uploadedFile, 'gallery');

                    // Update project record
                    $project->update(['thumbnail_url' => $upload['url']]);

                    // Cleanup temp file
                    @unlink($thumbnailPath);

                    $success++;
                } catch (\Throwable $e) {
                    $this->newLine();
                    $this->error("❌ Failed for: {$project->title} (ID: {$project->id}) — {$e->getMessage()}");
                    $failed++;
                }

                $bar->advance();
            }
        });

        $bar->finish();
        $this->newLine(2);

        $this->info("✅ Backfill complete: {$success} succeeded, {$failed} failed.");

        return $failed > 0 ? self::FAILURE : self::SUCCESS;
    }
}
